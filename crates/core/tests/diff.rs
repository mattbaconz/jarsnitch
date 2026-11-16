//! `diff` between fixture versions and in-memory archives.

mod common;

use common::*;
use jarsnitch_core::{Change, Diff, Item, Limits, Severity};

fn diff(old: &str, new: &str) -> Diff {
    jarsnitch_core::diff(
        &format!("{old}.jar"),
        &fixture(old),
        &format!("{new}.jar"),
        &fixture(new),
        &Limits::default(),
    )
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
    Change {
        kind,
        key: key.into(),
        old: Some(old.into()),
        new: Some(new.into()),
    }
}

#[test]
fn v2_adds_a_webhook_and_a_class_loader() {
    let d = diff("diff-v1", "diff-v2");
    assert_eq!(
        labels(&d.added),
        ["code.load", "str.discord-webhook=000000000000000000"]
    );
    assert!(d.removed.is_empty(), "{:?}", labels(&d.removed));
    assert_eq!(
        d.changed,
        [change(
            "plugin-field",
            "plugin.yml version",
            "1.0.0",
            "1.1.0"
        )]
    );
    let unchanged: Vec<_> = d
        .unchanged
        .iter()
        .map(|k| (k.rule.as_str(), k.value.as_deref()))
        .collect();
    assert_eq!(
        unchanged,
        [
            ("net.http", None),
            ("str.url", Some("api.example.invalid")),
            ("str.url", Some("discord.com"))
        ]
    );
    assert!(d.warnings.is_empty() && !d.adds_high() && !d.is_unreadable());
}

#[test]
fn going_back_removes_them() {
    let d = diff("diff-v2", "diff-v1");
    assert!(d.added.is_empty());
    assert_eq!(
        labels(&d.removed),
        ["code.load", "str.discord-webhook=000000000000000000"]
    );
}

#[test]
fn same_jar_has_no_changes() {
    for name in fixture_names() {
        let d = diff(&name, &name);
        assert!(
            d.added.is_empty() && d.removed.is_empty() && d.changed.is_empty(),
            "{name}"
        );
        assert!(
            d.same_code() && d.code.changed_files.is_empty() && d.warnings.is_empty(),
            "{name}: {:?}",
            d.code
        );
        assert_eq!(
            d.unchanged.len(),
            scan("x.jar", &fixture(&name)).findings.len(),
            "{name}"
        );
    }
}

#[test]
fn sample_update_adds_the_demo_findings() {
    // The update the web page's "Try a sample update" button compares.
    let d = diff("sample-v1", "sample-v2");
    let added = labels(&d.added);
    for rule in [
        "corr.download-and-run",
        "corr.hidden-command",
        "code.load",
        "mc.set-op",
    ] {
        assert!(
            added.iter().any(|l| l == rule),
            "missing {rule} in {added:?}"
        );
    }
    assert!(d.adds_high());
    // The old website is also gone as a host.
    assert_eq!(labels(&d.removed), ["str.url=sampleshop.example.com"]);
    let keys: Vec<_> = d.changed.iter().map(|c| c.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "plugin.yml authors",
            "plugin.yml version",
            "plugin.yml website"
        ]
    );
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
    assert!(
        added.contains(&r#"class:fixture/Main$Loader ["code.load"]"#.to_string()),
        "{added:?}"
    );
}

#[test]
fn metadata_changes() {
    let descriptor = |version: &str, command: &str, libraries: &str| {
        format!(
            "name: X\nversion: {version}\nmain: a.B\ncommands:\n  {command}:\n    description: x\nlibraries:\n{libraries}"
        )
    };
    let pom = |version: &str| {
        format!("groupId=com.google.code.gson\nartifactId=gson\nversion={version}\n")
    };
    let class = tiny_class();
    let pom_path = "META-INF/maven/com.google.code.gson/gson/pom.properties";
    let jar = |desc: &str, pom: &str, nested: &[u8]| {
        let inner = zip(&[("a.txt", nested)], false);
        zip(
            &[
                ("plugin.yml", desc.as_bytes()),
                ("a/B.class", &class),
                (pom_path, pom.as_bytes()),
                ("libs/x.jar", &inner),
            ],
            false,
        )
    };
    let old = jar(
        &descriptor("1.0", "hello", "  - org.xerial:sqlite-jdbc:3.46.1.0\n"),
        &pom("2.10.1"),
        b"one",
    );
    let new = jar(
        &descriptor(
            "1.1",
            "spawn",
            "  - org.xerial:sqlite-jdbc:3.49.1.0\n  - com.zaxxer:HikariCP:5.1.0\n",
        ),
        &pom("2.11.0"),
        b"two",
    );
    let d = diff_bytes(&old, &new);
    assert_eq!(
        labels(&d.added),
        ["command:spawn", "maven-library:com.zaxxer:HikariCP:5.1.0"]
    );
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
    assert_eq!(
        d.changed[0],
        change("library", "com.google.code.gson:gson", "2.10.1", "2.11.0")
    );
    assert_eq!(
        d.changed[1],
        change(
            "maven-library",
            "org.xerial:sqlite-jdbc",
            "3.46.1.0",
            "3.49.1.0"
        )
    );
    assert_ne!(d.changed[2].old, d.changed[2].new);
}

#[test]
fn author_website_and_dependency_changes() {
    let jar = |desc: &str| zip(&[("plugin.yml", desc.as_bytes())], false);
    let old = jar(
        "name: X\nauthor: Ann\nwebsite: https://a.example.com\ndepend: [Vault]\nsoftdepend: [Essentials]\n",
    );
    let new = jar(
        "name: X\nauthors: [Bo, Ann]\nwebsite: https://b.example.com\ndepend: [Vault, ProtocolLib]\n",
    );
    let d = diff_bytes(&old, &new);
    // The website is also a host the plugin names.
    assert_eq!(
        labels(&d.added),
        ["str.url=b.example.com", "dependency:ProtocolLib"]
    );
    assert_eq!(
        labels(&d.removed),
        ["str.url=a.example.com", "optional-dependency:Essentials"]
    );
    assert_eq!(
        d.changed,
        [
            change("plugin-field", "plugin.yml authors", "Ann", "Ann, Bo"),
            change(
                "plugin-field",
                "plugin.yml website",
                "https://a.example.com",
                "https://b.example.com"
            ),
        ]
    );
    let reordered = diff_bytes(
        &jar("name: X\nauthors: [A, B]\n"),
        &jar("name: X\nauthors: [B, A]\n"),
    );
    assert!(
        reordered.changed.is_empty(),
        "reordered authors aren't a change"
    );
}

#[test]
fn reobfuscated_jar_warns_instead_of_listing_classes() {
    let jar = |names: &[&str]| {
        let classes: Vec<(String, Vec<u8>)> = names
            .iter()
            .map(|n| (format!("{n}.class"), tiny_class_named(n)))
            .collect();
        let entries: Vec<(&str, &[u8])> = classes
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_slice()))
            .collect();
        zip(&entries, false)
    };
    let old = jar(&["a", "b", "c", "d", "e"]);
    assert!(scan("old.jar", &old).obfuscation.score >= 25);
    let d = diff_bytes(&old, &jar(&["f", "g", "h", "i", "j"]));
    assert_eq!(
        d.warnings
            .iter()
            .map(|w| w.code.as_str())
            .collect::<Vec<_>>(),
        ["diff.classes-renamed"]
    );
}

#[test]
fn unreadable_and_partial_sides() {
    let d = diff_bytes(b"not a zip", &fixture("benign-plugin"));
    assert!(d.is_unreadable());

    let jar = fixture("benign-plugin");
    let small = Limits {
        max_entries: 1,
        ..Limits::default()
    };
    let d = jarsnitch_core::diff("a.jar", &jar, "b.jar", &jar, &small);
    assert!(d.warnings.iter().any(|w| w.code == "diff.partial"));
    assert!(
        !d.same_code(),
        "a class the scan skipped was never compared"
    );
}

#[test]
fn injected_method_in_an_existing_class() {
    // inject-v2 is inject-v1 with one method added to Main and a call to it in onEnable.
    // javac renumbers Main's constant pool, so reload()'s bytes move but its code doesn't.
    let d = diff("inject-v1", "inject-v2");
    assert!(!d.same_code());
    let [main] = &d.code.changed_classes[..] else {
        panic!("{:?}", d.code)
    };
    assert_eq!(main.class, "fixture/Main");
    assert_eq!(main.severity, Severity::High);
    assert_eq!(main.rules_added, ["code.load", "corr.download-and-run"]);
    assert_eq!(main.methods_added, ["added$0()V"]);
    assert_eq!(main.methods_changed, ["onEnable()V"]);
    assert!(
        main.methods_removed.is_empty() && main.fields_added.is_empty() && !main.header_changed
    );
    assert!(
        main.calls_added
            .contains(&"java/net/URLClassLoader.loadClass".to_string()),
        "{:?}",
        main.calls_added
    );
    assert!(d.code.added_classes.is_empty() && d.code.changed_files.is_empty());
    assert_eq!(
        d.warnings
            .iter()
            .map(|w| w.code.as_str())
            .collect::<Vec<_>>(),
        ["diff.same-version"]
    );
}

#[test]
fn rezipped_and_resigned_jar_has_the_same_code() {
    let class = tiny_class();
    let desc: &[u8] = b"name: X
version: 1.0
main: a.B
";
    let old = zip(
        &[
            ("plugin.yml", desc),
            ("a/B.class", &class),
            (
                "META-INF/maven/x/y/pom.properties",
                b"#Mon
version=1",
            ),
        ],
        false,
    );
    // Other order, deflated, a signature added and a new build date in pom.properties.
    let new = zip(
        &[
            ("a/B.class", &class),
            ("META-INF/X.SF", b"sig"),
            (
                "META-INF/maven/x/y/pom.properties",
                b"#Tue
version=1",
            ),
            ("plugin.yml", desc),
        ],
        true,
    );
    let d = diff_bytes(&old, &new);
    assert!(
        d.same_code() && d.warnings.is_empty(),
        "{:?} {:?}",
        d.code,
        d.warnings
    );
    assert_eq!(d.code.unchanged_classes, 1);
    assert!(
        d.code.added_files.is_empty() && d.code.changed_files.is_empty(),
        "{:?}",
        d.code
    );

    // A changed descriptor is a changed file, not changed code.
    let other = zip(
        &[
            (
                "plugin.yml",
                b"name: X
version: 1.0
main: a.B
folia-supported: true
",
            ),
            ("a/B.class", &class),
        ],
        false,
    );
    let d = diff_bytes(&old, &other);
    assert!(d.same_code());
    assert_eq!(d.code.changed_files, ["plugin.yml"]);
}

#[test]
fn class_lists_cover_classes_without_findings() {
    let jar = |names: &[&str]| {
        let classes: Vec<(String, Vec<u8>)> = names
            .iter()
            .map(|n| (format!("{n}.class"), tiny_class_named(n)))
            .collect();
        let entries: Vec<(&str, &[u8])> = classes
            .iter()
            .map(|(p, c)| (p.as_str(), c.as_slice()))
            .collect();
        zip(&entries, false)
    };
    let d = diff_bytes(
        &jar(&["pkg/Alpha", "pkg/Beta"]),
        &jar(&["pkg/Alpha", "pkg/Gamma"]),
    );
    assert_eq!(d.code.added_classes, ["pkg/Gamma"]);
    assert_eq!(d.code.removed_classes, ["pkg/Beta"]);
    assert_eq!(d.code.unchanged_classes, 1);
    assert!(!d.same_code());
    // No class carries a finding, so the finding-based lists stay empty.
    assert!(d.added.is_empty() && d.removed.is_empty());
}

const DESC: &[u8] = b"name: X\nversion: 1.0\nmain: a.B\n";

fn jar_with(class: &[u8]) -> Vec<u8> {
    zip(&[("plugin.yml", DESC), ("a/B.class", class)], false)
}

#[test]
fn annotations_handlers_and_field_flags_are_code() {
    const CODE: &[u8] = &[0x00, 0x00, 0xb1]; // nop, nop, return
    let caught = |t: &'static str, handler: u16| [(0u16, 1u16, handler, Some(t))];
    let run = |handlers: &[Handler], annotation: Option<&str>| {
        let m = Member {
            code: Some((CODE, handlers)),
            annotation,
            ..Member::method("run", CODE)
        };
        class("a/B", &[Member::field(1, "x", "I")], &[m], None)
    };
    let base = run(&caught("java/lang/Exception", 2), None);
    let old = jar_with(&base);
    assert!(diff_bytes(&old, &jar_with(&base)).same_code());

    let methods = [
        (
            "catch type",
            run(&caught("java/lang/RuntimeException", 2), None),
        ),
        ("handler", run(&caught("java/lang/Exception", 1), None)),
        ("no handler", run(&[], None)),
        (
            "annotation",
            run(
                &caught("java/lang/Exception", 2),
                Some("Lorg/bukkit/event/EventHandler;"),
            ),
        ),
    ];
    for (what, new) in methods {
        let d = diff_bytes(&old, &jar_with(&new));
        assert!(!d.same_code(), "{what}");
        let [c] = &d.code.changed_classes[..] else {
            panic!("{what}: {:?}", d.code)
        };
        assert_eq!(c.methods_changed, ["run()V"], "{what}");
        assert!(!c.header_changed && c.fields_changed.is_empty(), "{what}");
    }

    let m = Member {
        code: Some((CODE, &caught("java/lang/Exception", 2))),
        ..Member::method("run", CODE)
    };
    let annotated = class(
        "a/B",
        &[Member::field(1, "x", "I")],
        &[m],
        Some("Lcom/example/Command;"),
    );
    let d = diff_bytes(&old, &jar_with(&annotated));
    assert!(!d.same_code() && d.code.changed_classes[0].header_changed);

    let m = Member {
        code: Some((CODE, &caught("java/lang/Exception", 2))),
        ..Member::method("run", CODE)
    };
    let shared = class("a/B", &[Member::field(9, "x", "I")], &[m], None); // public static
    let d = diff_bytes(&old, &jar_with(&shared));
    assert!(!d.same_code());
    assert_eq!(d.code.changed_classes[0].fields_changed, ["x:I"]);
}

#[test]
fn another_entry_with_the_same_name_is_not_the_same_code() {
    let official = class("a/B", &[], &[Member::method("run", &[0xb1])], None);
    let injected = class(
        "a/B",
        &[],
        &[Member::method("run", &[0x03, 0x57, 0xb1])],
        None,
    );
    let old = jar_with(&official);
    // The same name twice (the JVM reads the last), or names that clean to the same path.
    for path in ["a/B.class", "/a/B.class", "a\\B.class"] {
        let new = zip(
            &[
                ("plugin.yml", DESC),
                (path, &official),
                ("a/B.class", &injected),
            ],
            false,
        );
        let d = diff_bytes(&old, &new);
        assert!(!d.same_code(), "{path}");
        let [c] = &d.code.changed_classes[..] else {
            panic!("{path}: {:?}", d.code)
        };
        assert!(
            c.duplicate && c.severity >= Severity::Notable && !c.header_changed,
            "{path}: {c:?}"
        );
    }
    // Two copies of the same class are one class.
    let twice = zip(
        &[
            ("plugin.yml", DESC),
            ("a/B.class", &official),
            ("a/B.class", &official),
        ],
        false,
    );
    assert!(diff_bytes(&old, &twice).same_code());
    // The same two different copies in the other order are the same jar.
    let pq = zip(
        &[
            ("plugin.yml", DESC),
            ("a/B.class", &official),
            ("a/B.class", &injected),
        ],
        false,
    );
    let qp = zip(
        &[
            ("plugin.yml", DESC),
            ("a/B.class", &injected),
            ("a/B.class", &official),
        ],
        false,
    );
    assert!(diff_bytes(&pq, &qp).same_code());
}

#[test]
fn natives_main_and_libraries_are_code() {
    let class = tiny_class();
    let elf = |tail: &[u8]| [b"\x7fELF\x02\x01\x01".as_slice(), &[0; 57], tail].concat();
    let jar = |desc: &[u8], native: &[u8]| {
        zip(
            &[
                ("plugin.yml", desc),
                ("a/B.class", &class),
                ("natives/libx.so", native),
            ],
            false,
        )
    };
    let old = jar(DESC, &elf(b"a"));
    assert!(diff_bytes(&old, &jar(DESC, &elf(b"a"))).same_code());

    let d = diff_bytes(&old, &jar(DESC, &elf(b"b")));
    assert!(!d.same_code());
    assert!(
        d.warnings
            .iter()
            .any(|w| w.code == "diff.native-changed" && w.location == "natives/libx.so"),
        "{:?}",
        d.warnings
    );

    let main = b"name: X\nversion: 1.0\nmain: a.C\n";
    let libraries = b"name: X\nversion: 1.0\nmain: a.B\nlibraries:\n  - com.example:lib:1.0\n";
    for desc in [main.as_slice(), libraries] {
        assert!(
            !diff_bytes(&old, &jar(desc, &elf(b"a"))).same_code(),
            "{}",
            String::from_utf8_lossy(desc)
        );
    }
}

#[test]
fn snapshot() {
    insta::assert_json_snapshot!("diff-v1-to-v2", diff("diff-v1", "diff-v2"), { ".tool_version" => "[version]" });
}
