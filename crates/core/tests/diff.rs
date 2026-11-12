//! `diff` between fixture versions and in-memory archives.

mod common;

use common::*;
use jarsnitch_core::{Change, Diff, Item, Limits, Severity};

fn diff(old: &str, new: &str) -> Diff {
    jarsnitch_core::diff(&format!("{old}.jar"), &fixture(old), &format!("{new}.jar"), &fixture(new), &Limits::default())
}

fn diff_bytes(old: &[u8], new: &[u8]) -> Diff {
    jarsnitch_core::diff("old.jar", old, "new.jar", new, &Limits::default())
}

/// `rule` or `rule=value` for findings, `kind:value` for everything else.
fn labels(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .map(|i| match i {
            Item::Finding { finding } => match &finding.value {
                Some(v) => format!("{}={v}", finding.rule),
                None => finding.rule.clone(),
            },
            Item::Class { value, rules, .. } => format!("class:{value} {rules:?}"),
            Item::Command { value } => format!("command:{value}"),
            Item::Permission { value } => format!("permission:{value}"),
            Item::MavenLibrary { value } => format!("maven-library:{value}"),
            Item::Library { value } => format!("library:{value}"),
            Item::Dependency { value } => format!("dependency:{value}"),
            Item::OptionalDependency { value } => format!("optional-dependency:{value}"),
        })
        .collect()
}

fn change(kind: &'static str, key: &str, old: &str, new: &str) -> Change {
    Change { kind, key: key.into(), old: Some(old.into()), new: Some(new.into()) }
}

#[test]
fn v2_adds_a_webhook_and_a_class_loader() {
    let d = diff("diff-v1", "diff-v2");
    assert_eq!(labels(&d.added), ["code.load", "str.discord-webhook=000000000000000000"]);
    assert!(d.removed.is_empty(), "{:?}", labels(&d.removed));
    assert_eq!(d.changed, [change("plugin-field", "plugin.yml version", "1.0.0", "1.1.0")]);
    let unchanged: Vec<_> = d.unchanged.iter().map(|k| (k.rule.as_str(), k.value.as_deref())).collect();
    assert_eq!(unchanged, [("net.http", None), ("str.url", Some("api.example.invalid")), ("str.url", Some("discord.com"))]);
    assert!(d.warnings.is_empty() && !d.adds_high() && !d.is_unreadable());
}

#[test]
fn going_back_removes_them() {
    let d = diff("diff-v2", "diff-v1");
    assert!(d.added.is_empty());
    assert_eq!(labels(&d.removed), ["code.load", "str.discord-webhook=000000000000000000"]);
}

#[test]
fn same_jar_has_no_changes() {
    for name in fixture_names() {
        let d = diff(&name, &name);
        assert!(d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty(), "{name}");
        assert_eq!(d.unchanged.len(), scan("x.jar", &fixture(&name)).findings.len(), "{name}");
    }
}

#[test]
fn sample_update_adds_the_demo_findings() {
    // The update the web page's "Try a sample update" button compares.
    let d = diff("sample-v1", "sample-v2");
    let added = labels(&d.added);
    for rule in ["corr.download-and-run", "corr.hidden-command", "code.load", "mc.set-op"] {
        assert!(added.iter().any(|l| l == rule), "missing {rule} in {added:?}");
    }
    assert!(d.adds_high());
    // The old website is also gone as a host.
    assert_eq!(labels(&d.removed), ["str.url=sampleshop.example.com"]);
    let keys: Vec<_> = d.changed.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(keys, ["plugin.yml authors", "plugin.yml version", "plugin.yml website"]);
}

#[test]
fn added_high_finding() {
    let d = diff("code-load", "corr-download-and-run");
    assert!(d.adds_high());
    assert_eq!(d.added[0].severity(), Severity::High, "most severe first");
}

#[test]
fn new_class_with_notable_code() {
    let d = diff("exec-process", "code-load");
    let added = labels(&d.added);
    assert!(added.contains(&r#"class:fixture/Main$Loader ["code.load"]"#.to_string()), "{added:?}");
}

#[test]
fn metadata_changes() {
    let descriptor = |version: &str, command: &str, libraries: &str| {
        format!("name: X\nversion: {version}\nmain: a.B\ncommands:\n  {command}:\n    description: x\nlibraries:\n{libraries}")
    };
    let pom = |version: &str| format!("groupId=com.google.code.gson\nartifactId=gson\nversion={version}\n");
    let class = tiny_class();
    let pom_path = "META-INF/maven/com.google.code.gson/gson/pom.properties";
    let jar = |desc: &str, pom: &str, nested: &[u8]| {
        let inner = zip(&[("a.txt", nested)], false);
        zip(&[("plugin.yml", desc.as_bytes()), ("a/B.class", &class), (pom_path, pom.as_bytes()), ("libs/x.jar", &inner)], false)
    };
    let old = jar(&descriptor("1.0", "hello", "  - org.xerial:sqlite-jdbc:3.46.1.0\n"), &pom("2.10.1"), b"one");
    let new = jar(
        &descriptor("1.1", "spawn", "  - org.xerial:sqlite-jdbc:3.49.1.0\n  - com.zaxxer:HikariCP:5.1.0\n"),
        &pom("2.11.0"),
        b"two",
    );
    let d = diff_bytes(&old, &new);
    assert_eq!(labels(&d.added), ["command:spawn", "maven-library:com.zaxxer:HikariCP:5.1.0"]);
    assert_eq!(labels(&d.removed), ["command:hello"]);
    let changed: Vec<_> = d.changed.iter().map(|c| (c.kind, c.key.as_str())).collect();
    assert_eq!(
        changed,
        [
            ("library", "com.google.code.gson:gson"),
            ("maven-library", "org.xerial:sqlite-jdbc"),
            ("nested", "libs/x.jar"),
            ("plugin-field", "plugin.yml version"),
        ]
    );
    assert_eq!(d.changed[0], change("library", "com.google.code.gson:gson", "2.10.1", "2.11.0"));
    assert_eq!(d.changed[1], change("maven-library", "org.xerial:sqlite-jdbc", "3.46.1.0", "3.49.1.0"));
    assert_ne!(d.changed[2].old, d.changed[2].new);
}

#[test]
fn author_website_and_dependency_changes() {
    let jar = |desc: &str| zip(&[("plugin.yml", desc.as_bytes())], false);
    let old = jar("name: X\nauthor: Ann\nwebsite: https://a.example.com\ndepend: [Vault]\nsoftdepend: [Essentials]\n");
    let new = jar("name: X\nauthors: [Bo, Ann]\nwebsite: https://b.example.com\ndepend: [Vault, ProtocolLib]\n");
    let d = diff_bytes(&old, &new);
    // The website is also a host the plugin names.
    assert_eq!(labels(&d.added), ["str.url=b.example.com", "dependency:ProtocolLib"]);
    assert_eq!(labels(&d.removed), ["str.url=a.example.com", "optional-dependency:Essentials"]);
    assert_eq!(
        d.changed,
        [
            change("plugin-field", "plugin.yml authors", "Ann", "Ann, Bo"),
            change("plugin-field", "plugin.yml website", "https://a.example.com", "https://b.example.com"),
        ]
    );
    let reordered = diff_bytes(&jar("name: X\nauthors: [A, B]\n"), &jar("name: X\nauthors: [B, A]\n"));
    assert!(reordered.changed.is_empty(), "reordered authors aren't a change");
}

#[test]
fn reobfuscated_jar_warns_instead_of_listing_classes() {
    let jar = |names: &[&str]| {
        let classes: Vec<(String, Vec<u8>)> = names.iter().map(|n| (format!("{n}.class"), tiny_class_named(n))).collect();
        let entries: Vec<(&str, &[u8])> = classes.iter().map(|(p, c)| (p.as_str(), c.as_slice())).collect();
        zip(&entries, false)
    };
    let old = jar(&["a", "b", "c", "d", "e"]);
    assert!(scan("old.jar", &old).obfuscation.score >= 25);
    let d = diff_bytes(&old, &jar(&["f", "g", "h", "i", "j"]));
    assert_eq!(d.warnings.iter().map(|w| w.code.as_str()).collect::<Vec<_>>(), ["diff.classes-renamed"]);
}

#[test]
fn unreadable_and_partial_sides() {
    let d = diff_bytes(b"not a zip", &fixture("benign-plugin"));
    assert!(d.is_unreadable());

    let jar = fixture("benign-plugin");
    let small = Limits { max_entries: 1, ..Limits::default() };
    let d = jarsnitch_core::diff("a.jar", &jar, "b.jar", &jar, &small);
    assert!(d.warnings.iter().any(|w| w.code == "diff.partial"));
}

#[test]
fn snapshot() {
    insta::assert_json_snapshot!("diff-v1-to-v2", diff("diff-v1", "diff-v2"), { ".tool_version" => "[version]" });
}
