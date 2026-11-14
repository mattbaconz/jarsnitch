//! Every committed fixture jar: snapshot of its report, plus the rules it must and must
//! not trigger. Every rule must be covered here or in `robustness.rs`.

mod common;

use common::*;
use jarsnitch_core::{RULES, Report, Severity};

/// Fixture name, rules it must trigger, rules it must not trigger.
const EXPECT: &[(&str, &[&str], &[&str])] = &[
    ("benign-plugin", &["net.http", "mc.console-command", "str.url"], &[]),
    ("blob-embedded-code", &["blob.embedded-code", "exec.process"], &["corr.hidden-loader", "obf.score"]),
    ("code-instrument", &["code.instrument"], &[]),
    ("code-load", &["code.load"], &["corr.download-and-run"]),
    ("code-native", &["code.native"], &[]),
    ("corr-command-and-network", &["corr.command-and-network", "exec.process", "net.http"], &[]),
    ("corr-download-and-run", &["corr.download-and-run", "code.load", "net.http"], &[]),
    ("corr-download-and-run-hidden", &["corr.download-and-run", "code.load", "reflect.indirect", "str.ip", "str.hidden-text"], &[]),
    ("corr-download-and-run-negative", &["code.load", "net.http"], &["corr.download-and-run"]),
    ("corr-download-and-run-split", &["corr.download-and-run", "code.load", "net.http"], &[]),
    ("corr-login-files-and-network", &["corr.login-files-and-network", "str.login-file", "net.http"], &[]),
    ("str-hidden-text", &["str.hidden-text", "str.ip", "str.url"], &["corr.download-and-run"]),
    ("str-login-file", &["str.login-file"], &["corr.login-files-and-network"]),
    ("corr-hidden-command", &["corr.hidden-command", "mc.set-op"], &[]),
    ("corr-hidden-command-negative", &["mc.set-op"], &["corr.hidden-command"]),
    ("corr-hidden-loader", &["corr.hidden-loader", "blob.embedded-code", "code.load"], &[]),
    ("corr-modify-plugins", &["corr.modify-plugins", "fs.write-archive"], &[]),
    ("corr-modify-plugins-negative", &["fs.write-archive"], &["corr.modify-plugins"]),
    ("deser-java", &["deser.java"], &[]),
    ("diff-v1", &["net.http", "str.url"], &["code.load", "str.discord-webhook"]),
    ("diff-v2", &["net.http", "str.url", "code.load", "str.discord-webhook"], &["corr.download-and-run"]),
    ("exec-process", &["exec.process"], &["corr.command-and-network"]),
    ("fs-write-archive", &["fs.write-archive"], &["corr.modify-plugins"]),
    ("lib-log4j-vulnerable", &["lib.log4j-vulnerable"], &[]),
    ("mc-console-command", &["mc.console-command"], &[]),
    ("mc-set-op", &["mc.set-op"], &[]),
    ("meta-missing-main", &["meta.missing-main"], &[]),
    ("meta-no-descriptor", &["meta.no-descriptor"], &[]),
    ("net-http", &["net.http"], &[]),
    ("net-socket", &["net.socket"], &[]),
    ("obf-score", &["obf.score"], &[]),
    ("proc-exit", &["proc.exit"], &[]),
    ("reach-dispatch", &["str.url"], &[]),
    ("reach-order", &["str.url"], &[]),
    ("reach-paths", &["str.url"], &[]),
    ("reflect-indirect", &["reflect.indirect", "exec.process"], &[]),
    ("sample-v1", &["mc.console-command", "net.http", "str.url", "net.socket"], &["corr.download-and-run", "corr.hidden-command"]),
    ("sample-v2", &["corr.download-and-run", "corr.hidden-command", "code.load", "mc.set-op", "net.socket"], &[]),
    ("script-engine", &["script.engine"], &[]),
    ("str-discord-webhook", &["str.discord-webhook", "str.url"], &[]),
    ("str-ip", &["str.ip"], &[]),
    ("str-telegram-bot", &["str.telegram-bot"], &[]),
    ("str-url", &["str.url"], &[]),
];

/// Rules covered by in-memory archives in `robustness.rs` instead of a fixture jar.
const IN_MEMORY: &[&str] = &["res.type-mismatch", "jar.nested", "jar.unsafe-path"];

#[test]
fn every_rule_has_a_test() {
    for rule in RULES {
        let covered = IN_MEMORY.contains(&rule.id) || EXPECT.iter().any(|(_, must, _)| must.contains(&rule.id));
        assert!(covered, "rule {} has no fixture or in-memory test", rule.id);
    }
}

#[test]
fn every_fixture_jar_has_expectations() {
    for name in fixture_names() {
        assert!(EXPECT.iter().any(|(n, _, _)| *n == name), "fixtures/jars/{name}.jar has no entry in EXPECT");
    }
}

#[test]
fn fixtures_trigger_their_rules() {
    for (name, must, must_not) in EXPECT {
        let report = scan(&format!("{name}.jar"), &fixture(name));
        let found = rules(&report);
        for rule in *must {
            assert!(found.contains(rule), "{name}: expected {rule}, got {found:?}");
        }
        for rule in *must_not {
            assert!(!found.contains(rule), "{name}: did not expect {rule}");
        }
        assert!(!report.partial && report.warnings.is_empty(), "{name}: {:?}", report.warnings);
    }
}

#[test]
fn benign_fixture_has_no_high_findings() {
    let report = scan("benign-plugin.jar", &fixture("benign-plugin"));
    let high: Vec<_> = report.findings.iter().filter(|f| f.severity == Severity::High).map(|f| &f.rule).collect();
    assert!(high.is_empty(), "benign fixture has High findings: {high:?}");
    assert_eq!(report.plugins[0].commands, ["hello"]);
    assert_eq!(report.plugins[0].permissions, ["hello.use"]);
}

#[test]
fn detail_checks() {
    let r = scan("code-load.jar", &fixture("code-load"));
    let load = r.findings.iter().find(|f| f.rule == "code.load").unwrap();
    assert!(load.evidence.iter().any(|e| e.detail.contains("subclass of java/lang/ClassLoader")), "{:?}", load.evidence);

    let r = scan("str-ip.jar", &fixture("str-ip"));
    assert_eq!(values(&r, "str.ip"), ["2001:db8::1", "203.0.113.7"]);

    let r = scan("str-url.jar", &fixture("str-url"));
    assert_eq!(values(&r, "str.url"), ["api.example.invalid", "mc.example.net", "play.example.com", "updates.example.invalid"]);

    let r = scan("str-discord-webhook.jar", &fixture("str-discord-webhook"));
    let hook = r.findings.iter().find(|f| f.rule == "str.discord-webhook").unwrap();
    assert!(!hook.evidence[0].detail.contains("xxxxx"), "token must be redacted: {}", hook.evidence[0].detail);

    let r = scan("blob-embedded-code.jar", &fixture("blob-embedded-code"));
    let exec = r.findings.iter().find(|f| f.rule == "exec.process").unwrap();
    assert_eq!(exec.evidence[0].location, "fixture/Main!embedded-1!/payload/Payload");

    let r = scan("net-http.jar", &fixture("net-http"));
    let net = r.findings.iter().find(|f| f.rule == "net.http").unwrap();
    assert!(net.evidence.iter().all(|e| e.location == "fixture/Main"), "local URL reported: {:?}", net.evidence);

    let r = scan("deser-java.jar", &fixture("deser-java"));
    let deser = r.findings.iter().find(|f| f.rule == "deser.java").unwrap();
    assert!(deser.evidence.iter().all(|e| e.location == "fixture/Main"), "serialization hook reported: {:?}", deser.evidence);

    let r = scan("reflect-indirect.jar", &fixture("reflect-indirect"));
    let exec = r.findings.iter().find(|f| f.rule == "exec.process").unwrap();
    assert_eq!(exec.evidence[0].detail, "via reflection: java/lang/Runtime.exec");

    let r = scan("mc-console-command.jar", &fixture("mc-console-command"));
    let console = r.findings.iter().find(|f| f.rule == "mc.console-command").unwrap();
    let details: Vec<_> = console.evidence.iter().map(|e| e.detail.as_str()).collect();
    for want in [
        "org/bukkit/Bukkit.dispatchCommand",
        "org/bukkit/Server.dispatchCommand",
        "net/md_5/bungee/api/plugin/PluginManager.dispatchCommand",
        "com/velocitypowered/api/command/CommandManager.executeAsync",
    ] {
        assert!(details.contains(&want), "missing {want} in {details:?}");
    }
}

#[test]
fn reachability() {
    let r = scan("reach-paths.jar", &fixture("reach-paths"));
    let reach = r.reachability.as_ref().expect("plugin with a main class");
    assert_eq!(reach.entry_points, ["fixture/Main"]);
    assert!(!reach.dynamic);
    let reached: Vec<_> = r.findings.iter().filter(|f| f.rule == "str.url").map(|f| (f.value.as_deref().unwrap(), f.reached)).collect();
    assert_eq!(
        reached,
        [
            ("by-name.example.invalid", Some(true)),
            ("lambda.example.invalid", Some(true)),
            ("library.example.invalid", Some(false)),
            ("listener.example.invalid", Some(true)),
            ("never-made.example.invalid", Some(false)),
            ("static-init.example.invalid", Some(true)),
            ("tostring.example.invalid", Some(true)),
            ("unused.example.invalid", Some(false)),
            ("virtual.example.invalid", Some(true)),
        ]
    );

    // Parts in two other classes, joined by one method that calls both.
    let r = scan("corr-download-and-run-split.jar", &fixture("corr-download-and-run-split"));
    let corr = r.findings.iter().find(|f| f.rule == "corr.download-and-run").unwrap();
    assert!(corr.evidence.iter().all(|e| e.location == "fixture/Main" && e.method.as_deref() == Some("unused()V")), "{:?}", corr.evidence);
    for part in ["(in fixture/Fetch.get)", "(in fixture/Load.run)"] {
        assert!(corr.evidence.iter().any(|e| e.detail.ends_with(part)), "missing {part}: {:?}", corr.evidence);
    }

    // The web page's sample: the update's High findings are reachable, the unused library isn't.
    let r = scan("sample-v2.jar", &fixture("sample-v2"));
    let reached = |rule: &str| r.findings.iter().find(|f| f.rule == rule).and_then(|f| f.reached);
    assert_eq!(reached("corr.download-and-run"), Some(true));
    assert_eq!(reached("corr.hidden-command"), Some(true));
    assert_eq!(reached("net.socket"), Some(false));

    // One call path per reached finding, entry point first, each step saying how the one
    // before leads to it.
    let path = |r: &Report, rule: &str, value: Option<&str>| -> Vec<String> {
        let f = r.findings.iter().find(|f| f.rule == rule && f.value.as_deref() == value).unwrap();
        f.path.iter().map(|s| format!("{}.{} {:?}", s.location, &s.method[..s.method.find('(').unwrap()], s.via)).collect()
    };
    assert_eq!(path(&r, "mc.set-op", None), ["example/sampleshop/SampleShop.onEnable Entry", "example/sampleshop/ChatListener.onChat Callback"]);
    assert_eq!(path(&r, "corr.download-and-run", None), ["example/sampleshop/SampleShop.onEnable Entry", "example/sampleshop/AddonLoader.load Call"]);
    // Class-wide evidence takes the class's shortest path that isn't a constructor.
    assert_eq!(path(&r, "corr.hidden-command", None), ["example/sampleshop/SampleShop.onEnable Entry", "example/sampleshop/ChatListener.onChat Callback"]);
    assert!(path(&r, "net.socket", None).is_empty());
    let r = scan("reach-paths.jar", &fixture("reach-paths"));
    for (host, last) in [
        ("by-name.example.invalid", "fixture/ByName.host Reflection"),
        ("lambda.example.invalid", "fixture/Main.lambda$onEnable$0 Call"),
        ("listener.example.invalid", "fixture/Events.onJoin Callback"),
        ("static-init.example.invalid", "fixture/Config.<clinit> StaticInit"),
        ("tostring.example.invalid", "fixture/Overrider.toString Callback"),
        ("virtual.example.invalid", "fixture/TaskImpl.run Call"),
    ] {
        assert_eq!(path(&r, "str.url", Some(host)), ["fixture/Main.onEnable Entry", last], "{host}");
    }
    assert!(path(&r, "str.url", Some("unused.example.invalid")).is_empty());

    // The step before is the code that makes the call or lists the methods, not the one
    // that happens to create the object, whichever the scan meets first.
    let r = scan("reach-order.jar", &fixture("reach-order"));
    let entry = "fixture/Main.onEnable Entry";
    let listing = "fixture/Main.register Call";
    for (host, want) in [
        ("late-call.example.invalid", vec![entry, "fixture/TaskImpl.run Call"]),
        ("supertype.example.invalid", vec![entry, "fixture/Base.foo Call"]),
        ("listed.example.invalid", vec![entry, listing, "fixture/Plain.secret Reflection"]),
        ("listed-late.example.invalid", vec![entry, listing, "fixture/Late.secret Reflection"]),
        ("listed-static.example.invalid", vec![entry, listing, "fixture/Commands.helper Reflection"]),
    ] {
        assert_eq!(path(&r, "str.url", Some(host)), want, "{host}");
    }
    let shared = path(&r, "str.url", Some("shared.example.invalid"));
    assert_eq!(shared.last().map(String::as_str), Some("fixture/Shared.shared Call"), "{shared:?}");

    // A call on an interface lands on a method the created class inherits from a superclass
    // that doesn't implement the interface, whether the call or the `new` comes first.
    let r = scan("reach-dispatch.jar", &fixture("reach-dispatch"));
    assert_eq!(path(&r, "str.url", Some("inherited.example.invalid")), [entry, "fixture/AbstractModule.start Call"]);
    assert_eq!(path(&r, "str.url", Some("inherited-late.example.invalid")), [entry, "fixture/Main.serveAll Call", "fixture/AbstractService.serve Call"]);

    // No plugin main class: nothing to start from.
    let r = scan("meta-no-descriptor.jar", &fixture("meta-no-descriptor"));
    assert!(r.reachability.is_none() && r.findings.iter().all(|f| f.reached.is_none()));
}

#[test]
fn snapshots() {
    for name in fixture_names() {
        let report = scan(&format!("{name}.jar"), &fixture(&name));
        insta::assert_json_snapshot!(name.clone(), report, { ".tool_version" => "[version]" });
    }
}
