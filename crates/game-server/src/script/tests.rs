//! Unit tests of the script module (`docs/plans/ext-5-lua.md` section 12, step 1): the sandbox, the limits, the registrations, the
//! faults, the options. No world is needed: the API of this step is registration and reads.

use std::sync::Arc;
use std::time::{Duration, Instant};

use mlua::FromLua;
use serde_json::json;

use super::*;

fn input(name: &str, source: &str) -> ExtInput {
    ExtInput { name: name.to_owned(), scripts: vec![("main.lua".to_owned(), source.to_owned())], options: Vec::new() }
}

fn load(source: &str) -> Result<ExtScript> {
    ExtScript::load(&input("t", source), &LoadContext::default(), DEFAULT_MEMORY_MB, false)
}

fn eval<R: FromLua>(ext: &ExtScript, code: &str) -> R {
    ext.run("test", Limit::EVENT, |lua| lua.load(code).eval::<R>()).expect("the chunk runs")
}

#[test]
fn removed_names_are_nil_and_the_safe_ones_work() {
    let ext = load("").unwrap();
    for name in [
        "dofile",
        "loadfile",
        "load",
        "loadstring",
        "require",
        "module",
        "getfenv",
        "setfenv",
        "newproxy",
        "string.dump",
        "io",
        "package",
        "debug",
        "coroutine",
        "os.execute",
        "os.getenv",
        "os.remove",
        "os.exit",
        "os.rename",
        "os.tmpname",
        "os.setlocale",
    ] {
        assert!(eval::<bool>(&ext, &format!("return {name} == nil")), "{name} must not exist");
    }
    assert!(eval::<bool>(&ext, "return os.time() > 1000000000 and type(os.date('%H')) == 'string' and os.difftime(5, 3) == 2 and os.clock() >= 0"));
    assert_eq!(eval::<String>(&ext, "return string.format('%05.1f', 3.14159) .. ('ab'):upper() .. #('x'):rep(3)"), "003.1AB3");
    assert_eq!(eval::<i64>(&ext, "local t = {3, 1, 2} table.sort(t) return t[1] * 100 + t[2] * 10 + t[3] + math.floor(2.7)"), 125);
    assert!(eval::<bool>(&ext, "return collectgarbage('count') > 0"));
    let refused = ext.run("test", Limit::EVENT, |lua| lua.load("collectgarbage('collect')").exec());
    assert!(refused.is_err(), "only collectgarbage('count') is offered");
    assert_eq!(eval::<u32>(&ext, "return game.api"), 1);
    assert_eq!(eval::<String>(&ext, "return game.extension"), "t");
    assert_eq!(eval::<String>(&ext, "return print == game.log and 'same' or 'other'"), "same");
}

#[test]
fn a_binary_chunk_is_refused() {
    let err = load("\u{1b}Lua\u{51}\u{0}").err().expect("refused").to_string();
    assert!(err.contains("binary"), "{err}");
}

#[test]
fn an_endless_loop_ends_within_its_limit_at_the_load_in_an_event_and_in_a_point() {
    let err = load("while true do end").err().expect("the load is cut").to_string();
    assert!(err.contains("extension t") && err.contains("main.lua") && (err.contains("ran over") || err.contains("took longer")), "{err}");

    let ext = load("game.on('login', function() while true do end end) game.on('exp', function() while true do end end)").unwrap();
    let t = Instant::now();
    let err = ext.call::<_, ()>("login", Limit::EVENT, ()).err().expect("an error").to_string();
    assert!(err.contains("extension t: login:") && (err.contains("ran over 1000000") || err.contains("took longer than 20")), "{err}");
    let err = ext.call::<_, ()>("exp", Point::Exp.limit(), ()).err().expect("an error").to_string();
    assert!(err.contains("extension t: exp:") && (err.contains("ran over 200000") || err.contains("took longer than 5")), "{err}");
    assert!(t.elapsed() < Duration::from_secs(1), "both calls end in a moment");
}

#[test]
fn pcall_cannot_swallow_the_limit() {
    let ext = load("game.on('login', function() while true do pcall(function() while true do end end) end end)").unwrap();
    let t = Instant::now();
    let err = ext.call::<_, ()>("login", Limit::EVENT, ()).err();
    assert!(err.is_some(), "a loop that catches every error still ends");
    assert!(t.elapsed() < Duration::from_secs(2));
    let ext = load("game.on('login', function() while true do xpcall(function() while true do end end, function() end) end end)").unwrap();
    assert!(ext.call::<_, ()>("login", Limit::EVENT, ()).is_err());
}

#[test]
fn a_huge_string_is_a_memory_error_and_disables_the_extension() {
    let ext = load("game.on('login', function() return string.rep('x', 2^30) end)").unwrap();
    let err = ext.call::<_, String>("login", Limit::EVENT, ()).err().expect("memory error").to_string();
    assert!(err.contains("extension t: login:"), "{err}");
    assert!(ext.is_disabled(), "one memory error disables");
    assert!(ext.disabled_reason().unwrap().contains("a memory error"));
    // Disabled: nothing runs.
    assert_eq!(ext.call::<_, ()>("login", Limit::EVENT, ()), Ok(None));
    assert!(ext.run("test", Limit::EVENT, |lua| lua.load("return 1").eval::<i64>()).is_err(), "nor is anything else");
}

#[test]
fn the_budget_is_per_call() {
    let ext = load("").unwrap();
    for _ in 0..3 {
        assert!(ext.run("test", Limit::EVENT, |lua| lua.load("while true do end").exec()).is_err());
        // A handler within its limit runs again and again after a runaway.
        assert_eq!(ext.run("test", Limit::EVENT, |lua| lua.load("local n = 0 for i = 1, 100000 do n = n + 1 end return n").eval::<i64>()), Ok(100000));
    }
    assert!(!ext.is_disabled(), "three faults do not disable");
}

#[test]
fn five_faults_in_a_minute_disable_and_four_do_not() {
    let ext = load("game.on('login', function() error('boom') end)").unwrap();
    for n in 1..=4 {
        let err = ext.call::<_, ()>("login", Limit::EVENT, ()).err().expect("an error").to_string();
        assert!(err.contains("boom"), "{err}");
        assert!(!ext.is_disabled(), "{n} faults");
    }
    assert_eq!(ext.fault_count(), 4);
    assert!(ext.call::<_, ()>("login", Limit::EVENT, ()).is_err());
    assert!(ext.is_disabled(), "the fifth fault disables");
    assert!(ext.disabled_reason().unwrap().contains("5 errors in 60 s"));

    // Faults more than a minute apart do not add up.
    let ext = load("").unwrap();
    let base = Instant::now();
    let err = mlua::Error::runtime("x");
    for n in 0..8u64 {
        ext.fault_at(base + Duration::from_secs(n * 20), "login", &err);
    }
    assert!(!ext.is_disabled(), "one fault every 20 s never reaches 5 within 60 s");
    for _ in 0..5 {
        ext.fault_at(base + Duration::from_secs(1000), "login", &err);
    }
    assert!(ext.is_disabled());
}

#[test]
fn the_disabled_callback_is_told_once() {
    let ext = load("").unwrap();
    let told = Arc::new(Mutex::new(Vec::new()));
    let t = told.clone();
    ext.set_on_disable(Box::new(move |name, reason| t.lock().unwrap().push(format!("{name}: {reason}"))));
    ext.disable("first".into());
    ext.disable("second".into());
    assert_eq!(told.lock().unwrap().as_slice(), ["t: first"]);
}

#[test]
fn registrations_after_the_load_are_refused_but_timers_are_not() {
    let ext = load(
        r#"
        game.on('login', function()
          game.on('death', function() end)
        end)
        game.command('late', function(p, args)
          game.command('later', function() end)
        end)
        game.on('logout', function()
          local id = game.after(30, function() end)
          assert(game.cancel(id) == true)
          assert(game.cancel(id) == false)
          game.every(10, function() end)
        end)
        "#,
    )
    .unwrap();
    let err = ext.call::<_, ()>("login", Limit::EVENT, ()).err().expect("refused").to_string();
    assert!(err.contains("while the scripts load") && err.contains("main.lua:3"), "{err}");
    assert!(ext.call::<_, ()>("logout", Limit::EVENT, ()).unwrap().is_some());
    assert_eq!(ext.timer_count(), 1, "the timer set after the load stands");
}

#[test]
fn a_second_handler_for_one_name_is_a_load_error() {
    let err = load("game.on('login', function() end)\ngame.on('login', function() end)").err().expect("refused").to_string();
    assert!(err.contains("main.lua:2") && err.contains("already has a handler"), "{err}");
    for (code, what) in [
        ("game.on('nothing', function() end)", "not an event or point"),
        ("game.on('item_use', function() end)", "game.on_item"),
        ("game.on('command', function() end)", "game.command"),
        ("game.on('login', 5)", "second argument is a function"),
    ] {
        let err = load(code).err().expect("refused").to_string();
        assert!(err.contains(what), "{code}: {err}");
    }
}

#[test]
fn command_words_and_item_ids_are_validated() {
    for (code, what) in [
        ("game.command('Bad', function() end)", "not a command word"),
        ("game.command('has space', function() end)", "not a command word"),
        ("game.command('averyveryverylongword', function() end)", "not a command word"),
        ("game.command('x', function() end, { staff = 11 })", "staff is 0"),
        ("game.command('x', function() end, { staff = 1.5 })", "staff is 0"),
        ("game.command('x', function() end, { colour = 1 })", "the options are"),
        ("game.command('x', function() end) game.command('x', function() end)", "registered twice"),
        ("game.on_item({'1'}, function() end)", "not \"type/type_id\""),
        ("game.on_item({'0/1'}, function() end)", "not \"type/type_id\""),
        ("game.on_item({'1/256'}, function() end)", "not \"type/type_id\""),
        ("game.on_item({}, function() end)", "1 to"),
        ("game.on_item({'1/1', '1/1'}, function() end)", "listed twice"),
        ("game.on_item({'1/1'}, function() end) game.on_item({'1/1'}, function() end)", "taken over twice"),
        ("game.on_item('1/1', function() end)", "list"),
        ("game.every(0, function() end)", "whole number"),
        ("game.every(86401, function() end)", "whole number"),
        ("game.after(1.5, function() end)", "whole number"),
    ] {
        let err = load(code).err().unwrap_or_else(|| panic!("{code} should be refused")).to_string();
        assert!(err.contains(what), "{code}: {err}");
    }
    let ok = load("game.command('good_1', function() end, { staff = 3, help = 'does a thing' }) game.on_item({'1/2', '3/4'}, function() end)").unwrap();
    assert_eq!(ok.commands()[0].1.staff, 3);
    assert_eq!(ok.commands()[0].1.help, "does a thing");
    assert_eq!(ok.items(), vec![(1, 2), (3, 4)]);

    // An item that is not in the definitions.
    let exists: Arc<dyn Fn(u8, u8) -> bool + Send + Sync> = Arc::new(|t, i| (t, i) == (1, 2));
    let ctx = LoadContext { item_exists: Some(exists), ..LoadContext::default() };
    let err = ExtScript::load(&input("t", "game.on_item({'1/2', '9/9'}, function() end)"), &ctx, 16, false).err().expect("refused").to_string();
    assert!(err.contains("item 9/9 does not exist"), "{err}");
    assert!(ExtScript::load(&input("t", "game.on_item({'1/2'}, function() end)"), &ctx, 16, false).is_ok());
}

#[test]
fn two_extensions_cannot_share_a_command_or_an_item() {
    let a = input("a", "game.command('x', function() end) game.on_item({'1/1'}, function() end) game.on('login', function() end)");
    let b = input("b", "game.command('x', function() end)");
    let c = input("c", "game.on_item({'1/1'}, function() end)");
    let d = input("d", "game.command('y', function() end) game.on('login', function() end) game.on('exp', function() end)");
    let set = ScriptSet::load(&[a, b, c, d], LoadContext::default(), 16);
    assert_eq!(set.extensions().iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["a", "d"]);
    let failed: Vec<&str> = set.failed().iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(failed, ["b", "c"]);
    assert!(set.failed()[0].1.contains("another extension already registered !x"), "{}", set.failed()[0].1);
    assert!(set.failed()[1].1.contains("another extension already takes over item 1/1"));
    assert_eq!(set.command("x").unwrap().0.name, "a");
    assert_eq!(set.command("y").unwrap().0.name, "d");
    assert!(set.command("z").is_none());
    assert_eq!(set.item_owner(1, 1).unwrap().name, "a");
    assert!(set.item_owner(1, 2).is_none());
    assert_eq!(set.for_event(Event::Login).iter().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["a", "d"], "in install order");
    assert!(set.for_event(Event::Death).is_empty());
    assert_eq!(set.for_point(Point::Exp).len(), 1);
    assert!(ScriptSet::empty().for_point(Point::Chat).is_empty());
}

#[test]
fn check_names_the_file_and_line_of_a_syntax_error() {
    let mut inp = input("t", "local ok = 1\n");
    inp.scripts.push(("lib/more.lua".to_owned(), "local a = 1\nlocal = = 2\n".to_owned()));
    let err = check(&inp, None).err().expect("refused").to_string();
    assert!(err.contains("t/lib/more.lua:2"), "{err}");
    // A top-level error names the line too.
    let err = check(&input("t", "local x = 1\nerror('nope')"), None).err().expect("refused").to_string();
    assert!(err.contains("t/main.lua:2") && err.contains("nope"), "{err}");
    // Files load in order into one state: the second sees the first's globals.
    let mut inp = input("t", "shared = 41");
    inp.scripts.push(("two.lua".to_owned(), "game.on('login', function() return shared + 1 end)".to_owned()));
    let summary = check(&inp, None).unwrap();
    assert_eq!(summary.events, ["login"]);
}

#[test]
fn check_summarises_what_the_scripts_register() {
    let src = r#"
        game.on('login', function() end)
        game.on('kill', function() end)
        game.on('chat', function() end)
        game.on('enchant', function() end)
        game.command('b', function() end, { staff = 2 })
        game.command('a', function() end)
        game.on_item({'5/6'}, function() end)
        game.every(60, function() end)
        game.after(5, function() end)
        game.log('hello from the load')
    "#;
    let summary = check(&input("t", src), None).unwrap();
    assert_eq!(summary.events, ["kill", "login"]);
    assert_eq!(summary.points, ["chat", "enchant"]);
    assert_eq!(summary.commands, [("a".to_owned(), 0), ("b".to_owned(), 2)]);
    assert_eq!(summary.items, ["5/6"]);
    assert_eq!(summary.timers, 2);
    assert_eq!(summary.handlers(), 2 + 2 + 2 + 1);
    let text = summary.to_string();
    assert!(text.contains("commands: !a, !b (staff 2)") && text.contains("timers: 2"), "{text}");
    assert_eq!(check(&input("t", ""), None).unwrap().to_string().lines().next().unwrap(), "events: none");
}

#[test]
fn timers_come_due_once_and_one_shots_go() {
    let ext = load("game.every(10, function() end) game.after(5, function() end)").unwrap();
    assert_eq!(ext.timer_count(), 2);
    let now = Instant::now();
    assert!(ext.due_timers(now).is_empty());
    assert_eq!(ext.due_timers(now + Duration::from_secs(6)), vec![2], "the one-shot");
    assert_eq!(ext.timer_count(), 1, "it is gone");
    // A repeating timer very late runs once.
    let late = now + Duration::from_secs(1000);
    assert_eq!(ext.due_timers(late), vec![1]);
    assert!(ext.due_timers(late + Duration::from_secs(5)).is_empty(), "its next time is a period after");
    assert_eq!(ext.due_timers(late + Duration::from_secs(10)), vec![1]);
    assert!(ext.next_timer().unwrap() > late);
    // Timers are limited to 64.
    let err = load("for i = 1, 65 do game.every(10, function() end) end").err().expect("refused").to_string();
    assert!(err.contains("at most 64 timers"), "{err}");
}

#[test]
fn log_is_limited_to_twenty_a_second() {
    let ext = load("for i = 1, 25 do game.log('line', i) end print(1.5, true, nil)").unwrap();
    ext.shared().capture_log();
    // The load's lines were not captured; log again from an event.
    let ext2 = load("game.on('login', function() for i = 1, 25 do game.log('line ' .. i) end end)").unwrap();
    ext2.shared().capture_log();
    ext2.call::<_, ()>("login", Limit::EVENT, ()).unwrap();
    let lines = ext2.shared().captured();
    assert_eq!(lines.len(), 20, "the rest is dropped and counted");
    assert_eq!(lines[0], "line 1");
    // Long lines are cut at 1,000 bytes.
    let ext3 = load("game.on('login', function() game.log(string.rep('é', 800)) end)").unwrap();
    ext3.shared().capture_log();
    ext3.call::<_, ()>("login", Limit::EVENT, ()).unwrap();
    assert!(ext3.shared().captured()[0].len() <= 1000);
}

#[test]
fn the_state_and_the_registrations_do_not_keep_each_other_alive() {
    let ext = load("game.on('login', function() end) game.command('x', function() end) game.every(5, function() end)").unwrap();
    let weak = Arc::downgrade(ext.shared());
    drop(ext);
    assert!(weak.upgrade().is_none(), "dropping an extension frees its shared part (a reload does not leak states)");
}

fn spec(v: serde_json::Value) -> OptionSpec {
    OptionSpec::parse(&v).unwrap()
}

#[test]
fn option_specs_check_values_by_type_min_and_max() {
    let int = spec(json!({"type": "integer", "default": 5, "doc": "d", "min": 0, "max": 10}));
    assert!(int.check(&json!(0)).is_ok() && int.check(&json!(10)).is_ok());
    assert!(int.check(&json!(-1)).unwrap_err().to_string().contains("is below 0"));
    assert!(int.check(&json!(11)).unwrap_err().to_string().contains("is above 10"));
    assert!(int.check(&json!(1.5)).unwrap_err().to_string().contains("whole number"));
    assert!(int.check(&json!("5")).is_err() && int.check(&json!(true)).is_err());
    let float = spec(json!({"type": "float", "default": 0.5, "doc": "d", "min": 0, "max": 1}));
    assert!(float.check(&json!(0.25)).is_ok() && float.check(&json!(1)).is_ok());
    assert!(float.check(&json!(1.5)).is_err() && float.check(&json!("x")).is_err());
    let boolean = spec(json!({"type": "boolean", "default": true, "doc": "d"}));
    assert!(boolean.check(&json!(false)).is_ok() && boolean.check(&json!(1)).is_err());
    let string = spec(json!({"type": "string", "default": "hi", "doc": "d"}));
    assert!(string.check(&json!("plain ASCII ~")).is_ok());
    assert!(string.check(&json!("café")).unwrap_err().to_string().contains("printable ASCII"));
    assert!(string.check(&json!("x".repeat(1001))).unwrap_err().to_string().contains("at most 1000"));
    assert!(string.check(&json!(5)).is_err());
    let strings = spec(json!({"type": "strings", "default": [], "doc": "d"}));
    assert!(strings.check(&json!(["a", "b"])).is_ok() && strings.check(&json!(["a", 1])).is_err() && strings.check(&json!("a")).is_err());
    assert!(strings.check(&json!(vec!["x"; 257])).unwrap_err().to_string().contains("at most 256"));
    let ints = spec(json!({"type": "integers", "default": [1, 2], "doc": "d"}));
    assert!(ints.check(&json!([3, -4])).is_ok() && ints.check(&json!([1.5])).is_err());
    let maps = spec(json!({"type": "maps", "default": [], "doc": "d"}));
    assert!(maps.check(&json!([35, "40-42", 0, 65535])).is_ok());
    for bad in [json!([65536]), json!(["5-3"]), json!(["a-b"]), json!(["70000-70001"]), json!([-1]), json!([true]), json!(5)] {
        assert!(maps.check(&bad).is_err(), "{bad}");
    }
}

#[test]
fn option_specs_are_validated() {
    for (v, what) in [
        (json!({"type": "integer", "doc": "d"}), "default is required"),
        (json!({"default": 1, "doc": "d"}), "type is required"),
        (json!({"type": "integer", "default": 1}), "doc is required"),
        (json!({"type": "number", "default": 1, "doc": "d"}), "is not one of"),
        (json!({"type": "integer", "default": "x", "doc": "d"}), "default:"),
        (json!({"type": "integer", "default": 50, "doc": "d", "max": 10}), "default: 50 is above 10"),
        (json!({"type": "string", "default": "", "doc": "d", "min": 1}), "integer and float options only"),
        (json!({"type": "integer", "default": 1, "doc": "d", "min": 1.5}), "whole"),
        (json!({"type": "integer", "default": 1, "doc": "d", "min": 5, "max": 2}), "above max"),
        (json!({"type": "integer", "default": 1, "doc": "d", "step": 1}), "unknown key"),
        (json!({"type": "integer", "default": 1, "doc": "two\nlines"}), "one line"),
        (json!("integer"), "is a table"),
    ] {
        let err = OptionSpec::parse(&v).err().unwrap_or_else(|| panic!("{v} should be refused")).to_string();
        assert!(err.contains(what), "{v}: {err}");
    }
    let s = spec(json!({"type": "integer", "default": 3, "doc": "d", "min": 0, "max": 9}));
    assert_eq!(OptionSpec::parse(&s.to_json()).unwrap(), s, "a spec survives its JSON");
    assert!(check_option(&json!({"type": "boolean", "default": true, "doc": "d"}), &json!(false)).is_ok());
    assert!(check_option(&json!({"type": "boolean", "default": true, "doc": "d"}), &json!("no")).is_err());
    assert!(options::valid_key("bounty_gold") && !options::valid_key("Bounty") && !options::valid_key("_x") && !options::valid_key(""));
}

#[test]
fn game_options_holds_the_values_with_maps_as_a_set_and_is_read_only() {
    let specs = vec![
        ("maps".to_owned(), spec(json!({"type": "maps", "default": [35, "40-42"], "doc": "d"}))),
        ("gold".to_owned(), spec(json!({"type": "integer", "default": 100000, "doc": "d", "min": 0}))),
        ("rate".to_owned(), spec(json!({"type": "float", "default": 0.5, "doc": "d"}))),
        ("text".to_owned(), spec(json!({"type": "string", "default": "Hello {name}", "doc": "d"}))),
        ("on".to_owned(), spec(json!({"type": "boolean", "default": true, "doc": "d"}))),
        ("steps".to_owned(), spec(json!({"type": "integers", "default": [5, 10], "doc": "d"}))),
        ("empty".to_owned(), spec(json!({"type": "maps", "default": [], "doc": "d"}))),
    ];
    let mut inp = input("t", "");
    inp.options = options::defaults(&specs);
    inp.options[1].value = json!(250);
    let ext = ExtScript::load(&inp, &LoadContext::default(), 16, false).unwrap();
    assert!(eval::<bool>(&ext, "local o = game.options return o.maps[35] == true and o.maps[41] == true and o.maps[43] == nil and o.maps[36] == nil"));
    assert_eq!(eval::<i64>(&ext, "return game.options.gold"), 250);
    assert_eq!(eval::<f64>(&ext, "return game.options.rate"), 0.5);
    assert_eq!(eval::<String>(&ext, "return game.options.text"), "Hello {name}");
    assert!(eval::<bool>(&ext, "return game.options.on"));
    assert_eq!(eval::<i64>(&ext, "local s = 0 for _, v in ipairs(game.options.steps) do s = s + v end return s + #game.options.steps"), 17);
    assert!(eval::<bool>(&ext, "return next(game.options.empty) == nil"), "no maps: the set is empty");
    assert!(eval::<bool>(&ext, "return game.options.nothing == nil"));
    let err = ext.run("test", Limit::EVENT, |lua| lua.load("game.options.gold = 1").exec()).err().expect("refused").to_string();
    assert!(err.contains("read-only"), "{err}");
    assert!(eval::<bool>(&ext, "return getmetatable(game.options) == false"), "the metatable is hidden");
}

#[test]
fn a_wrong_answer_type_is_a_fault() {
    let ext = load("game.on('exp', function() return 'two' end)").unwrap();
    let r = ext.call::<_, Option<f64>>("exp", Point::Exp.limit(), ());
    assert!(r.is_err(), "a string where a number is wanted");
    assert_eq!(ext.fault_count(), 1);
    assert_eq!(ext.call::<_, Option<f64>>("login", Limit::EVENT, ()), Ok(None), "no handler: nothing asked");
}

#[test]
fn event_and_point_names_round_trip() {
    for e in Event::ALL {
        assert_eq!(Event::from_name(e.name()), Some(e));
    }
    for p in Point::ALL {
        assert_eq!(Point::from_name(p.name()), Some(p));
    }
    assert_eq!(Event::from_name("exp"), None);
    assert_eq!(Point::from_name("login"), None);
    assert_eq!(api::parse_item("12/34"), Some((12, 34)));
    assert_eq!(api::parse_item("0/1"), None);
    assert_eq!(api::parse_item("1/0"), None);
    assert_eq!(api::parse_item("a/1"), None);
    assert_eq!(api::parse_item("1/2/3"), None);
}

// ----------------------------------------------------------------------------------------------------------------- the layers (step 3)

mod layers {
    use super::*;
    use crate::config::{Layer, read_file};
    use crate::script::sources::{FileOptions, ScriptExtension, ScriptSources, resolve_options};

    fn ext(name: &str, specs: &[(&str, serde_json::Value)]) -> ScriptExtension {
        ScriptExtension {
            name: name.to_owned(),
            scripts: vec![("main.lua".to_owned(), String::new())],
            options: specs.iter().map(|(k, v)| ((*k).to_owned(), OptionSpec::parse(v).unwrap())).collect(),
            globals: Vec::new(),
        }
    }

    fn sources(file: Option<&str>) -> ScriptSources {
        let mut s = ScriptSources {
            extensions: vec![
                ext(
                    "kill-streak",
                    &[
                        ("bounty_gold", json!({"type": "integer", "default": 100000, "doc": "d", "min": 0, "max": 100000000})),
                        ("announce_at", json!({"type": "integers", "default": [5, 10], "doc": "d"})),
                        ("maps", json!({"type": "maps", "default": [], "doc": "d"})),
                    ],
                ),
                ext("welcome", &[("text", json!({"type": "string", "default": "Welcome, {name}!", "doc": "d"}))]),
            ],
            installed: vec![("kill-streak".to_owned(), true), ("welcome".to_owned(), true), ("off".to_owned(), false), ("plain".to_owned(), true)],
            ..ScriptSources::default()
        };
        if let Some(text) = file {
            let layer = read_file("config/game.toml", text).unwrap();
            s.file = Some(FileOptions { path: layer.path, options: layer.options });
        }
        s
    }

    fn value<'a>(r: &'a crate::script::sources::Resolution, ext: &str, key: &str) -> (&'a serde_json::Value, &'a Layer) {
        let e = r.extensions.iter().find(|e| e.extension == ext).unwrap();
        let (o, layer) = e.options.iter().find(|(o, _)| o.key == key).unwrap();
        (&o.value, layer)
    }

    #[test]
    fn each_layer_lies_over_the_one_below() {
        let mut s = sources(Some("guild_party_size = 2\n\n[ext.kill-streak]\nbounty_gold = 5000\nannounce_at = [3]\n\n[ext.welcome]\ntext = \"Hi {name}\"\n"));
        let r = resolve_options(&s).unwrap();
        assert_eq!((value(&r, "kill-streak", "bounty_gold"), value(&r, "kill-streak", "maps")), ((&json!(5000), &Layer::File), (&json!([]), &Layer::Default)));
        assert_eq!(value(&r, "kill-streak", "announce_at"), (&json!([3]), &Layer::File));
        assert_eq!(value(&r, "welcome", "text"), (&json!("Hi {name}"), &Layer::File));
        // The editor beats the file.
        s.editor = vec![("ext.kill-streak.bounty_gold".to_owned(), "7".to_owned(), Some(14)), ("ext.kill-streak.maps".to_owned(), "[35,\"40-42\"]".to_owned(), None)];
        let r = resolve_options(&s).unwrap();
        assert_eq!(value(&r, "kill-streak", "bounty_gold"), (&json!(7), &Layer::Editor(Some(14))));
        assert_eq!(value(&r, "kill-streak", "maps"), (&json!([35, "40-42"]), &Layer::Editor(None)));
        assert_eq!(value(&r, "kill-streak", "announce_at"), (&json!([3]), &Layer::File), "untouched keys keep their layer");
        assert!(r.warnings.is_empty());
        // The loader's input carries the values in force.
        let inputs = r.inputs(&s);
        assert_eq!(inputs.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["kill-streak", "welcome"]);
        assert_eq!(inputs[0].options.iter().find(|o| o.key == "bounty_gold").unwrap().value, json!(7));
        // `game-server config` lists them with their layers.
        let shown = r.show();
        assert!(shown.contains("[ext.kill-streak]") && shown.contains("bounty_gold = 7") && shown.contains("# data editor (change set 14)"), "{shown}");
        assert!(shown.contains("announce_at = [3]") && shown.contains("# config file"), "{shown}");
        assert!(shown.contains("text = \"Hi {name}\"") && shown.contains("# config file"), "{shown}");
        assert!(shown.contains("maps = [35, \"40-42\"]") && shown.contains("# data editor\n"), "{shown}");
        let defaults = resolve_options(&sources(None)).unwrap().show();
        assert!(defaults.contains("bounty_gold = 100000") && defaults.contains("# extension default") && !defaults.contains("config file"), "{defaults}");
        // The options as Lua sees them.
        let mut input = inputs.into_iter().next().unwrap();
        input.scripts = vec![("main.lua".to_owned(), "game.on('login', function() return game.options.bounty_gold + #game.options.announce_at end)".to_owned())];
        let loaded = ExtScript::load(&input, &LoadContext::default(), 16, false).unwrap();
        assert_eq!(loaded.call::<_, i64>("login", Limit::EVENT, ()).unwrap(), Some(8));
    }

    #[test]
    fn errors_name_the_place_and_the_key() {
        let cases = [
            ("[ext.kill-streak]\nbounty_gold = -5\n", "config/game.toml line 2: ext.kill-streak.bounty_gold: -5 is below 0"),
            ("\n\n[ext.kill-streak]\nbounty_gold = 200000000\n", "config/game.toml line 4: ext.kill-streak.bounty_gold: 200000000 is above 100000000"),
            ("[ext.kill-streak]\nbounty_gold = \"lots\"\n", "config/game.toml line 2: ext.kill-streak.bounty_gold: \"lots\" is not a whole number"),
            ("[ext.kill-streak]\nbounty_gold = 1.5\n", "ext.kill-streak.bounty_gold: 1.5 is not a whole number"),
            ("[ext.kill-streak]\nannounce_at = [1, \"x\"]\n", "line 2: ext.kill-streak.announce_at: \"x\" is not a whole number"),
            ("[ext.kill-streak]\nmaps = [\"9-3\"]\n", "line 2: ext.kill-streak.maps: \"9-3\" is not a map range"),
            ("[ext.kill-streak]\nnothing = 1\n", "config/game.toml line 2: ext.kill-streak.nothing: the extension kill-streak has no option \"nothing\" (its options: bounty_gold, announce_at, maps)"),
            ("[ext.plain]\nx = 1\n", ""),
            ("[ext.welcome]\ntext = 5\n", "line 2: ext.welcome.text: 5 is not a string"),
            ("[ext.welcome]\ntext = \"caf\\u00e9\"\n", "ext.welcome.text: \"café\" holds a character that is not printable ASCII"),
        ];
        for (file, expect) in cases {
            let result = resolve_options(&sources(Some(file)));
            if expect.is_empty() {
                // An enabled extension without scripts is not in the set: a warning.
                assert!(result.unwrap().warnings[0].contains("has no scripts"));
                continue;
            }
            let error = result.unwrap_err().to_string();
            assert!(error.contains(expect), "{file:?}: {error}");
        }
        let mut s = sources(None);
        s.editor = vec![("ext.kill-streak.bounty_gold".to_owned(), "-1".to_owned(), Some(14))];
        assert_eq!(resolve_options(&s).unwrap_err().to_string(), "data editor (change set 14): ext.kill-streak.bounty_gold: -1 is below 0");
        s.editor = vec![("ext.welcome.text".to_owned(), "plain words".to_owned(), Some(3))];
        assert!(resolve_options(&s).unwrap_err().to_string().starts_with("data editor (change set 3): ext.welcome.text: the value \"plain words\" is not JSON"));
        s.editor = vec![("ext.welcome".to_owned(), "1".to_owned(), None)];
        assert!(resolve_options(&s).unwrap_err().to_string().contains("not an extension option"));
    }

    #[test]
    fn layers_for_an_extension_that_does_not_count_are_warnings() {
        let mut s = sources(Some("[ext.off]\nx = 1\n[ext.gone]\ny = 2\n[ext.welcome]\ntext = \"ok\"\n"));
        s.editor = vec![
            ("ext.off.x".to_owned(), "1".to_owned(), Some(2)),
            ("ext.welcome.removed_option".to_owned(), "1".to_owned(), None),
            ("ext.gone.y".to_owned(), "1".to_owned(), None),
        ];
        let r = resolve_options(&s).unwrap();
        assert_eq!(value(&r, "welcome", "text"), (&json!("ok"), &Layer::File), "the rest still counts");
        let w = r.warnings.join("\n");
        assert!(w.contains("config/game.toml line 2: [ext.off] is ignored: the extension off is disabled"), "{w}");
        assert!(w.contains("line 4: [ext.gone] is ignored: the extension gone is not installed"), "{w}");
        assert!(w.contains("data editor (change set 2): ext.off.x is ignored: the extension off is disabled"), "{w}");
        assert!(w.contains("ext.welcome.removed_option is ignored: the extension welcome has no option \"removed_option\" (its options: text)"), "{w}");
        assert_eq!(r.warnings.len(), 5);
    }

    #[test]
    fn the_file_reads_extension_tables_with_their_lines() {
        let layer = read_file("g.toml", "guild_party_size = 2\n# c\n[ext.a-b]\nn = 3\ns = \"x\"\nl = [1, \"2-3\"]\nb = true\nf = 0.5\n\n[ext.cd]\nq = 1\n").unwrap();
        assert_eq!(layer.values.len(), 1);
        let got: Vec<(String, String, serde_json::Value, usize)> = layer.options.iter().map(|o| (o.extension.clone(), o.key.clone(), o.value.clone(), o.line)).collect();
        assert_eq!(
            got,
            [
                ("a-b".to_owned(), "b".to_owned(), json!(true), 7),
                ("a-b".to_owned(), "f".to_owned(), json!(0.5), 8),
                ("a-b".to_owned(), "l".to_owned(), json!([1, "2-3"]), 6),
                ("a-b".to_owned(), "n".to_owned(), json!(3), 4),
                ("a-b".to_owned(), "s".to_owned(), json!("x"), 5),
                ("cd".to_owned(), "q".to_owned(), json!(1), 11),
            ],
            "keys in the table's order, each with its line"
        );
        assert!(read_file("g.toml", "").unwrap().options.is_empty());
    }

    #[test]
    fn the_script_settings_have_defaults_and_ranges() {
        use crate::config::{Sources, Startup, resolve};
        let startup = Startup { bind: "0.0.0.0:1".parse().unwrap(), database_url: "x".into(), server_id: 1, config: None };
        let (config, _) = resolve(&startup, &Sources::default()).unwrap();
        assert!(config.scripts);
        assert_eq!(config.script_memory_mb, 16);
        let file = |text: &str| Sources { file: Some(read_file("g.toml", text).unwrap()), ..Sources::default() };
        let (config, all) = resolve(&startup, &file("scripts = false\nscript_memory_mb = 64\n")).unwrap();
        assert!(!config.scripts);
        assert_eq!(config.script_memory_mb, 64);
        assert_eq!(all.iter().find(|r| r.name == "scripts").unwrap().layer, Layer::File);
        for bad in ["0", "257", "-1"] {
            assert!(resolve(&startup, &file(&format!("script_memory_mb = {bad}\n"))).is_err(), "{bad}");
        }
        assert!(crate::config::check_setting("script_memory_mb", "many").is_err());
        assert!(crate::config::check_setting("script_memory_mb", "256").is_ok() && crate::config::check_setting("script_memory_mb", "1").is_ok());
    }
}

// ------------------------------------------------------------------------------------------------------ reads and acting (step 4)

mod acting {
    use super::*;
    use crate::script::event::{Killer, MobInfo, Payload, PlayerRef};
    use crate::script::world::{PlayerView, ScriptWorld};
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct Fake {
        sent: StdMutex<Vec<String>>,
    }

    fn view(id: u32, name: &str) -> PlayerView {
        PlayerView { id, name: name.to_owned(), level: 40 + id as u16, country: (id % 2) as u8, map: 1, position: [1.0, 2.5, 3.0], kills: 7, job: 2, staff: 0, guild: (id == 1).then_some(9) }
    }

    impl ScriptWorld for Fake {
        fn player(&self, id: u32) -> Option<PlayerView> {
            (1..=3).contains(&id).then(|| view(id, ["", "Alice", "Bob", "Carol"][id as usize]))
        }
        fn find(&self, name: &str) -> Option<PlayerView> {
            (1..=3).filter_map(|i| self.player(i)).find(|v| v.name.eq_ignore_ascii_case(name))
        }
        fn online(&self, max: usize) -> Vec<PlayerView> {
            (1..=3).filter_map(|i| self.player(i)).take(max).collect()
        }
        fn online_count(&self) -> usize {
            3
        }
        fn party(&self, id: u32) -> Vec<u32> {
            if id == 1 || id == 2 { vec![1, 2] } else { vec![id] }
        }
        fn whisper(&self, id: u32, text: &str) {
            self.sent.lock().unwrap().push(format!("whisper {id}: {text}"));
        }
        fn notice_to(&self, id: u32, text: &str) {
            self.sent.lock().unwrap().push(format!("notice {id}: {text}"));
        }
        fn notice_all(&self, text: &str) {
            self.sent.lock().unwrap().push(format!("all: {text}"));
        }
        fn notice_faction(&self, country: u8, text: &str) {
            self.sent.lock().unwrap().push(format!("faction {country}: {text}"));
        }
        fn notice_map(&self, map: u16, text: &str) {
            self.sent.lock().unwrap().push(format!("map {map}: {text}"));
        }
        fn banner(&self, name: &str, text: &str) {
            self.sent.lock().unwrap().push(format!("banner {name}: {text}"));
        }
        fn item_name(&self, t: u8, i: u8) -> Option<String> {
            ((t, i) == (25, 1)).then(|| "Potion".to_owned())
        }
        fn mob_name(&self, id: u16) -> Option<String> {
            (id == 7).then(|| "Boar".to_owned())
        }
        fn tell_staff(&self, text: &str) {
            self.sent.lock().unwrap().push(format!("staff: {text}"));
        }
        fn commands(&self) -> Vec<(String, String, u8)> {
            vec![("hi".into(), "says hi".into(), 0)]
        }
    }

    fn with_world(source: &str) -> (ExtScript, Arc<Fake>) {
        let fake = Arc::new(Fake::default());
        let ctx = LoadContext { world: Some(fake.clone()), ..LoadContext::default() };
        (ExtScript::load(&input("t", source), &ctx, 16, false).unwrap(), fake)
    }

    #[test]
    fn the_reads_give_what_the_world_says() {
        let (ext, _) = with_world("");
        assert_eq!(eval::<String>(&ext, "local p = game.player(1) return p.name .. ' ' .. p.id .. ' ' .. p:level() .. ' ' .. p:faction() .. ' ' .. p:map() .. ' ' .. p:kills() .. ' ' .. p:job() .. ' ' .. p:staff()"), "Alice 1 41 fury 1 7 2 0");
        assert_eq!(eval::<String>(&ext, "local x, y, z = game.player(2):position() return x .. ',' .. y .. ',' .. z"), "1,2.5,3");
        assert_eq!(eval::<String>(&ext, "return game.player(2):faction()"), "light");
        assert_eq!(eval::<i64>(&ext, "return game.player(1):guild()"), 9);
        assert!(eval::<bool>(&ext, "return game.player(2):guild() == nil and game.player(99) == nil and game.player(-1) == nil and game.player(1.5) == nil"));
        assert!(eval::<bool>(&ext, "return game.find('bOB').id == 2 and game.find('nobody') == nil"));
        assert_eq!(eval::<i64>(&ext, "return #game.online() * 10 + game.online_count()"), 33);
        assert!(eval::<bool>(&ext, "return game.player(1) == game.find('alice') and game.player(1) ~= game.player(2) and tostring(game.player(3)) == 'Carol'"), "handles compare by id");
        assert_eq!(eval::<i64>(&ext, "local m = game.player(2):party() return #m * 100 + m[1].id * 10 + m[2].id"), 212);
        assert_eq!(eval::<i64>(&ext, "return #game.player(3):party()"), 1);
        assert!(eval::<bool>(&ext, "return game.player(1):online() and game.player(1):has_feature('x') == false"));
        assert!(eval::<bool>(&ext, "return game.item_name(25, 1) == 'Potion' and game.item_name(1, 1) == nil and game.mob_name(7) == 'Boar' and game.mob_name(8) == nil"));
        assert_eq!(eval::<String>(&ext, "local c = game.commands()[1] return c.word .. c.help .. c.staff"), "hisays hi0");
        // An offline player reads nil (the handle of a player that the world no longer has).
        let offline = "local p = game.player(1) p.id = 77 return p:level() == nil and p:map() == nil and p:online() == false";
        assert!(eval::<bool>(&ext, offline));
    }

    #[test]
    fn messages_and_notices_follow_the_text_rules() {
        let (ext, fake) = with_world("");
        let run = |code: &str| ext.run("t", Limit::EVENT, |lua| lua.load(code).exec());
        run("local p = game.player(2) p:message('hello') p:notice('Hi\\200there') game.notice_all('news') game.notice_faction('fury', 'f') game.notice_map(35, 'm') game.banner('b') game.banner('c', 'Boss')").unwrap();
        assert_eq!(
            fake.sent.lock().unwrap().as_slice(),
            ["whisper 2: hello", "notice 2: Hi?there", "all: news", "faction 1: f", "map 35: m", "banner Notice: b", "banner Boss: c"],
            "a byte outside 0x20 to 0x7E becomes ?"
        );
        fake.sent.lock().unwrap().clear();
        // A long message is cut into whisper lines of 120 bytes.
        run(&format!("game.player(2):message(string.rep('x', 250))")).unwrap();
        let sent = fake.sent.lock().unwrap().clone();
        assert_eq!(sent.iter().map(|s| s.len() - "whisper 2: ".len()).collect::<Vec<_>>(), [120, 120, 10]);
        for (code, expect) in [
            ("game.player(2):notice(string.rep('x', 128))", "at most 127"),
            ("game.player(2):message(string.rep('x', 1201))", "at most 1200"),
            ("game.notice_all(string.rep('x', 2000))", "at most 1999"),
            ("game.notice_map(1, string.rep('x', 128))", "at most 127"),
            ("game.banner(string.rep('x', 128))", "at most 127"),
            ("game.banner('x', string.rep('n', 21))", "at most 20"),
            ("game.player(2):message('<<999c01 crash')", "colour marker"),
            ("game.player(2):message({})", "is a string"),
            ("game.notice_faction('both', 'x')", "\"light\" or \"fury\""),
        ] {
            // A fault each: a state of its own keeps them from adding up to a disabled extension.
            let (fresh, _) = with_world("");
            let error = fresh.run("t", Limit::EVENT, |lua| lua.load(code).exec()).err().unwrap_or_else(|| panic!("{code} should fail")).to_string();
            assert!(error.contains(expect), "{code}: {error}");
        }
    }

    #[test]
    fn rates_limit_notices_to_many_and_messages_to_one() {
        let (ext, fake) = with_world("");
        let run = |code: &str| ext.run("t", Limit::EVENT, |lua| lua.load(code).exec());
        // Five messages a second to one player; the sixth is a Lua error (a fault), another player is not limited.
        run("for i = 1, 5 do game.player(2):message('m' .. i) end").unwrap();
        let error = run("game.player(2):message('m6')").err().expect("limited").to_string();
        assert!(error.contains("at most 5 messages and notices a second to one player"), "{error}");
        run("game.player(3):message('other')").unwrap();
        assert_eq!(fake.sent.lock().unwrap().len(), 6);
        // Twenty notices to many a minute.
        run("for i = 1, 20 do game.notice_all('n' .. i) end").unwrap();
        let error = run("game.banner('one more')").err().expect("limited").to_string();
        assert!(error.contains("at most 20 notices and banners a minute"), "{error}");
        assert_eq!(ext.fault_count(), 2);
        assert!(!ext.is_disabled());
    }

    #[test]
    fn events_bring_their_arguments() {
        let (ext, fake) = with_world(
            "game.on('kill', function(p, mob, near)
               game.log(p.name .. ' ' .. mob.name .. ' ' .. mob.id .. ' ' .. mob.uid .. ' ' .. mob.level .. ' ' .. mob.map .. ' ' .. mob.x .. ',' .. mob.z .. ' ' .. tostring(mob.boss) .. ' ' .. tostring(mob.spawned) .. ' near ' .. #near .. ' ' .. near[2].name)
             end)
             game.on('pvp_kill', function(killer, victim, last, near) game.log('pvp ' .. killer.name .. ' ' .. victim.name .. ' ' .. tostring(last and last.name) .. ' ' .. #near) end)
             game.on('death', function(victim, by, killer, pvp) game.log('death ' .. victim.name .. ' ' .. by .. ' ' .. tostring(type(killer) == 'table' and killer.name or killer) .. ' ' .. tostring(pvp)) end)
             game.on('map_enter', function(p, map) game.log('enter ' .. p.name .. ' ' .. map) end)",
        );
        ext.shared().capture_log();
        let a = PlayerRef { id: 1, name: "Alice".into() };
        let b = PlayerRef { id: 2, name: "Bob".into() };
        let mob = MobInfo { id: 7, uid: 99, name: "Boar".into(), level: 12, map: 1, x: 5.0, z: 6.0, boss: false, spawned: None };
        let set = {
            let ext = Arc::new(ext);
            ext
        };
        let fire = |event: Event, payload: Payload| {
            let _ = set.call_built::<()>(event.name(), Limit::EVENT, |lua| crate::script::act::event_args(lua, &payload));
        };
        fire(Event::Kill, Payload::Kill { p: a.clone(), mob: mob.clone(), near: vec![a.clone(), b.clone()] });
        fire(Event::PvpKill, Payload::PvpKill { killer: a.clone(), victim: b.clone(), last_hit: None, near: vec![a.clone()] });
        fire(Event::Death, Payload::Death { victim: b.clone(), killer: Killer::Player(a.clone()), pvp: true });
        fire(Event::Death, Payload::Death { victim: b.clone(), killer: Killer::Monster(55), pvp: false });
        fire(Event::MapEnter, Payload::MapEnter { p: a, map: 35 });
        assert_eq!(
            set.shared().captured(),
            [
                "Alice Boar 7 99 12 1 5,6 false nil near 2 Bob",
                "pvp Alice Bob nil 1",
                "death Bob player Alice true",
                "death Bob monster 55 false",
                "enter Alice 35"
            ]
        );
        assert_eq!(set.fault_count(), 0);
        let _ = fake;
    }
}
