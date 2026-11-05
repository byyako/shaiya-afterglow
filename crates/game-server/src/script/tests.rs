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
    assert!(ext.disabled_reason().unwrap().contains("out of memory"));
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
    let set = ScriptSet::load(&[a, b, c, d], None, 16);
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
