//! `data-editor`: the page (`serve`) and the command line of the data editor (`docs/plans/done/data-editing.md` section 5).

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use data_editor::apply::Report;
use data_editor::store::{Applied, Line, Op, Refused};
use data_editor::{Editor, connect};
use serde_json::Value;

const USAGE: &str = "usage: data-editor <command>

  serve                              the page (EDITOR_BIND, default 127.0.0.1:30890)
  apply                              write all edits into defs and world (after every load of the original files)
  apply-draft -m MESSAGE [--by NAME] apply the draft as one change set
  status                             markers, counts of edits and draft, problems a dry run of apply finds
  history [N]                        the last N change sets (default 20)
  show <id>                          the changes of one change set
  undo <id> [--by NAME]              undo a change set
  export <file> [--images]           all edits as one file (--images adds the map pictures)
  import <file> [--replace]          load an export (--replace first reverts the edits in force)
  images import <folder>             map pictures (<map id>.png or .jpg with <map id>.json) and names.json
  draft list | clear | remove <n>    the draft
  draft add set <target> <key> <field> <json>
  draft add unset <target> <key> <field>
  draft add add <target> <key> <json object of every field>
  draft add remove <target> <key>
  content status                     what is imported and installed, and whether a client patch is due
  content hashes [schema...]         schema.table|rows|md5 per table (default: defs world client), the proof of a build
  content rebuild [defs|world|all] [--into PREFIX]
                                     defs and world again from the frozen originals, the extensions' rows and the edits
                                     (--into: into scratch schemas PREFIX_defs and PREFIX_world, rolled back)
  content check                      the frozen copies, the live tables against a rebuild, the extensions' ids and settings
  content origin <table> <key>       the layers of one row: original, extension, edits

Environment: DATABASE_URL (required), EDITOR_BIND (the page's address), EDITOR_EXPORT_FILE (written after every Apply and Undo when set),
EDITOR_HOSTS (extra host names a request may be addressed to; localhost, 127.0.0.1 and [::1] always may).";

fn take_flag(args: &mut Vec<String>, flag: &str) -> bool {
    let before = args.len();
    args.retain(|a| a != flag);
    args.len() != before
}

fn take_value(args: &mut Vec<String>, flags: &[&str]) -> Result<Option<String>> {
    let Some(at) = args.iter().position(|a| flags.contains(&a.as_str())) else { return Ok(None) };
    if at + 1 >= args.len() {
        bail!("{} needs a value", args[at]);
    }
    let value = args.remove(at + 1);
    args.remove(at);
    Ok(Some(value))
}

fn print_report(report: &Report) {
    for schema in &report.missing {
        println!("schema {schema} does not exist: nothing to write into");
    }
    for schema in &report.fresh {
        println!("schema {schema} is fresh (no marker): originals refreshed, edits written");
    }
    println!("{} table values written", report.written);
    for p in &report.problems {
        let at = format!("{} {}{}", p.target, p.key, p.field.as_ref().map(|f| format!(" {f}")).unwrap_or_default());
        let cs = p.change_set.map(|c| format!(" (change set #{c})")).unwrap_or_default();
        println!("PROBLEM {at}{cs}: {}", p.text);
    }
}

fn print_applied(what: &str, applied: &Applied) {
    for p in &applied.problems {
        println!("PROBLEM (writing the edits into freshly loaded tables first) {} {}: {}", p.target, p.key, p.text);
    }
    println!("{what} #{}: {} changes", applied.change_set, applied.changes);
    for effect in &applied.effects {
        let hint = match effect.name() {
            "live" | "next spawn" => "console <server id> reload",
            _ => "console <server id> restart <minutes>",
        };
        println!("  takes effect {}: {hint}", effect.name());
    }
}

async fn after_change(editor: &Editor) {
    if let Ok(path) = std::env::var("EDITOR_EXPORT_FILE")
        && !path.is_empty()
        && let Err(e) = editor.export_to(&PathBuf::from(&path), false).await
    {
        eprintln!("the change is applied, but the export to {path} failed: {e:#}");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else { bail!(USAGE) };
    args.remove(0);
    let result = run(&command, args).await;
    if let Err(e) = &result
        && let Some(Refused(m)) = e.downcast_ref::<Refused>()
    {
        eprintln!("refused: {m}");
        std::process::exit(2);
    }
    result
}

async fn run(command: &str, mut args: Vec<String>) -> Result<()> {
    if matches!(command, "help" | "-h" | "--help") {
        println!("{USAGE}");
        return Ok(());
    }
    let by = take_value(&mut args, &["--by"])?.unwrap_or_else(|| "cli".to_owned());
    let editor = connect().await?;
    match command {
        "apply" => {
            let report = editor.apply_edits().await?;
            print_report(&report);
        }
        "apply-draft" => {
            let message = take_value(&mut args, &["-m", "--message"])?.context("apply-draft needs -m MESSAGE")?;
            let applied = editor.apply_draft(&message, &by).await?;
            print_applied("change set", &applied);
            after_change(&editor).await;
        }
        "status" => {
            let status = editor.status().await?;
            let marker = |m: &Option<String>| m.clone().unwrap_or_else(|| "none (fresh or not applied yet)".to_owned());
            println!("defs marker:   {}", marker(&status.defs_marker));
            println!("world marker:  {}", marker(&status.world_marker));
            println!(
                "change sets: {}, edited fields: {}, rows added: {}, rows removed: {}, draft lines: {}",
                status.change_sets, status.field_edits, status.added_rows, status.removed_rows, status.draft_lines
            );
            let report = editor.apply_dry_run().await?;
            if report.problems.is_empty() {
                println!("a dry run of apply finds no problems");
            } else {
                print_report(&report);
            }
        }
        "history" => {
            let n: i64 = args.first().map(|a| a.parse()).transpose().context("history takes a number")?.unwrap_or(20);
            for h in editor.history(n).await? {
                let note = match (h.undoes, h.undone_by) {
                    (Some(u), _) => format!(" [undoes #{u}]"),
                    (None, Some(b)) => format!(" [undone by #{b}]"),
                    _ => String::new(),
                };
                println!("#{} {} {} ({} changes) {}{note}", h.id, h.made_at, h.made_by, h.changes, h.message);
            }
        }
        "show" => {
            let id: i32 = args.first().context("show needs a change set id")?.parse()?;
            for l in editor.history_detail(id).await? {
                let value = |v: &Option<Value>| v.as_ref().map(|v| v.to_string()).unwrap_or_else(|| "-".to_owned());
                let field = l.field.as_deref().map(|f| format!(" {f}")).unwrap_or_default();
                let (before, after) = (value(&l.before), value(&l.after));
                let clip = |s: String| if s.len() > 120 { format!("{}...", &s[..120]) } else { s };
                println!("{:>3} {} {}{field}: {} -> {}", l.seq, l.target, l.key, clip(before), clip(after));
            }
        }
        "undo" => {
            let id: i32 = args.first().context("undo needs a change set id")?.parse()?;
            let applied = editor.undo(id, &by).await?;
            print_applied("undo change set", &applied);
            after_change(&editor).await;
        }
        "export" => {
            let images = take_flag(&mut args, "--images");
            let path = args.first().context("export needs a file")?;
            editor.export_to(&PathBuf::from(path), images).await?;
            println!("exported to {path}");
        }
        "import" => {
            let replace = take_flag(&mut args, "--replace");
            let path = args.first().context("import needs a file")?;
            let doc = Editor::read_export(&PathBuf::from(path))?;
            let report = editor.import(&doc, replace).await?;
            print_report(&report);
            after_change(&editor).await;
        }
        "serve" => {
            tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
                .init();
            let bind =
                std::env::var("EDITOR_BIND").ok().filter(|b| !b.is_empty()).unwrap_or_else(|| data_editor::serve::DEFAULT_BIND.to_owned());
            let export_file = std::env::var("EDITOR_EXPORT_FILE").ok().filter(|p| !p.is_empty()).map(PathBuf::from);
            let hosts = data_editor::serve::parse_hosts(&std::env::var("EDITOR_HOSTS").unwrap_or_default());
            data_editor::serve::serve(editor, &bind, export_file, hosts).await?;
        }
        "images" => match args.as_slice() {
            [sub, folder] if sub == "import" => {
                for line in editor.import_images(&PathBuf::from(folder)).await? {
                    println!("{line}");
                }
            }
            _ => bail!("usage: data-editor images import <folder>"),
        },
        "draft" => draft(&editor, &args, &by).await?,
        "content" => content(&editor, args).await?,
        other => bail!("unknown command {other:?}\n\n{USAGE}"),
    }
    Ok(())
}

async fn content(editor: &Editor, mut args: Vec<String>) -> Result<()> {
    use data_editor::registry::Side;
    let sub = if args.is_empty() { String::new() } else { args.remove(0) };
    match sub.as_str() {
        "status" => {
            for line in editor.content_status().await? {
                println!("{line}");
            }
        }
        "hashes" => {
            for line in editor.content_hashes(&args).await? {
                println!("{line}");
            }
        }
        "rebuild" => {
            let into = take_value(&mut args, &["--into"])?;
            let parts = match args.first().map(String::as_str) {
                None | Some("all") => vec![Side::Defs, Side::World],
                Some("defs") => vec![Side::Defs],
                Some("world") => vec![Side::World],
                Some(other) => bail!("content rebuild takes defs, world or all, not {other:?}"),
            };
            let results = match &into {
                Some(prefix) => editor.content_rebuild_into(&parts, prefix).await?,
                None => editor.content_rebuild(&parts).await?,
            };
            for r in &results {
                let rows: i64 = r.tables.iter().map(|(_, n)| n).sum();
                let created = if r.created { " (the schema was created from its DDL)" } else { "" };
                println!("{}: {} tables, {rows} original rows, {} extension rows{created}", r.part, r.tables.len(), r.ext_rows);
                print_report(&r.report);
            }
            if into.is_some() {
                println!("(into scratch schemas: rolled back, nothing written)");
            }
        }
        "check" => {
            let findings = editor.content_check().await?;
            for f in &findings {
                println!("{f}");
            }
            if findings.is_empty() {
                println!("content check: clean");
            } else {
                println!("content check: {} findings", findings.len());
                std::process::exit(1);
            }
        }
        "origin" => match args.as_slice() {
            [table, key] => {
                for line in editor.content_origin(table, key).await? {
                    println!("{line}");
                }
            }
            _ => bail!("usage: data-editor content origin <table> <key>"),
        },
        _ => bail!("usage: data-editor content status | hashes [schema...] | rebuild [defs|world|all] [--into PREFIX] | check | origin <table> <key>"),
    }
    Ok(())
}

async fn draft(editor: &Editor, args: &[String], by: &str) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") | None => {
            for d in editor.draft_list().await? {
                let field = d.field.as_deref().map(|f| format!(" {f}")).unwrap_or_default();
                let value = d.value.as_ref().map(|v| format!(" = {v}")).unwrap_or_default();
                println!("{:>3} {} {} {}{field}{value}  ({})", d.seq, d.action, d.target, d.key, d.made_by);
            }
        }
        Some("clear") => println!("{} lines removed", editor.draft_clear().await?),
        Some("remove") => {
            let seq: i32 = args.get(1).context("draft remove needs a line number")?.parse()?;
            println!("{}", if editor.draft_remove(seq).await? { "removed" } else { "no such line" });
        }
        Some("add") => {
            let a: Vec<&str> = args[1..].iter().map(String::as_str).collect();
            let json = |text: &str| serde_json::from_str::<Value>(text).context("the value must be JSON");
            let line = match a.as_slice() {
                ["set", target, key, field, value] => {
                    Line { target: (*target).into(), key: (*key).into(), op: Op::Set { field: (*field).into(), value: json(value)? } }
                }
                ["unset", target, key, field] => {
                    Line { target: (*target).into(), key: (*key).into(), op: Op::Unset { field: (*field).into() } }
                }
                ["add", target, key, fields] => {
                    Line { target: (*target).into(), key: (*key).into(), op: Op::Add { fields: json(fields)? } }
                }
                ["remove", target, key] => Line { target: (*target).into(), key: (*key).into(), op: Op::Remove },
                _ => bail!("{USAGE}"),
            };
            println!("draft line {}", editor.draft_add(&line, by).await?);
        }
        Some(other) => bail!("unknown draft command {other:?}"),
    }
    Ok(())
}
