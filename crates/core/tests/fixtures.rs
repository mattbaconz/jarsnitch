//! Every committed fixture jar: snapshot of its report, plus the rules it must and must
//! not trigger. Every rule must be covered here or in `robustness.rs`.

mod common;

use common::*;
use jarsnitch_core::{RULES, Severity};

/// Fixture name, rules it must trigger, rules it must not trigger.
const EXPECT: &[(&str, &[&str], &[&str])] = &[
    ("benign-plugin", &["net.http", "mc.console-command", "str.url"], &[]),
    ("blob-embedded-code", &["blob.embedded-code", "exec.process"], &["corr.hidden-loader", "obf.score"]),
    ("code-instrument", &["code.instrument"], &[]),
    ("code-load", &["code.load"], &["corr.download-and-run"]),
    ("code-native", &["code.native"], &[]),
    ("corr-command-and-network", &["corr.command-and-network", "exec.process", "net.http"], &[]),
    ("corr-download-and-run", &["corr.download-and-run", "code.load", "net.http"], &[]),
    ("corr-download-and-run-negative", &["code.load", "net.http"], &["corr.download-and-run"]),
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
    ("reflect-indirect", &["reflect.indirect", "exec.process"], &[]),
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
fn snapshots() {
    for name in fixture_names() {
        let report = scan(&format!("{name}.jar"), &fixture(&name));
        insta::assert_json_snapshot!(name.clone(), report, { ".tool_version" => "[version]" });
    }
}
