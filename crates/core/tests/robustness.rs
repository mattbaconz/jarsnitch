//! Hostile and malformed archives built in memory. None of these may panic.

mod common;

use common::*;
use jarsnitch_core::Limits;

fn warning_codes(r: &jarsnitch_core::Report) -> Vec<&str> {
    r.warnings.iter().map(|w| w.code.as_str()).collect()
}

#[test]
fn not_a_zip() {
    let r = scan("x.jar", b"definitely not a zip");
    assert!(r.is_unreadable());
    assert!(r.findings.is_empty());
}

#[test]
fn truncated_and_shifted_jars() {
    let jar = fixture("benign-plugin");
    for cut in [0, 1, 21, jar.len() / 2, jar.len() - 1] {
        let _ = scan("cut.jar", &jar[..cut]);
    }
    let _ = scan("shifted.jar", &jar[100..]);
}

#[test]
fn mutated_fixtures_never_panic() {
    for name in fixture_names() {
        let jar = fixture(&name);
        for i in (0..jar.len()).step_by(11) {
            let mut bad = jar.clone();
            bad[i] ^= 0xA5;
            let _ = scan("mutated.jar", &bad);
        }
    }
}

#[test]
fn garbage_class_is_a_warning() {
    let mut class = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
    class.extend([0xff; 40]);
    let r = scan("x.jar", &zip(&[("a/B.class", &class)], false));
    assert!(warning_codes(&r).contains(&"class.malformed"));
}

#[test]
fn zip_bomb_hits_the_ratio_limit() {
    let zeros = vec![0u8; 8 << 20];
    let r = scan("bomb.jar", &zip(&[("bomb.txt", &zeros)], true));
    assert!(r.partial);
    assert!(warning_codes(&r).contains(&"zip.limit"), "{:?}", r.warnings);
}

#[test]
fn size_and_entry_limits() {
    let data = vec![b'a'; 600];
    let jar = zip(&[("a.txt", &data), ("b.txt", &data), ("c.txt", &data)], false);
    let small = Limits { max_total_bytes: 1000, ..Limits::default() };
    let r = jarsnitch_core::scan("x.jar", &jar, &small);
    assert!(r.partial && r.stats.uncompressed_bytes <= 1000);

    let few = Limits { max_entries: 2, ..Limits::default() };
    let r = jarsnitch_core::scan("x.jar", &jar, &few);
    assert!(r.partial && r.stats.entries <= 2);
}

#[test]
fn nested_jars_are_scanned_with_their_path() {
    let inner = fixture("exec-process");
    let outer = zip(&[("libs/inner.jar", &inner)], true);
    let r = scan("outer.jar", &zip(&[("libs/middle.jar", &outer)], false));
    assert_eq!(values(&r, "jar.nested"), ["libs/middle.jar", "libs/middle.jar!/libs/inner.jar"]);
    let exec = r.findings.iter().find(|f| f.rule == "exec.process").unwrap();
    assert_eq!(exec.evidence[0].location, "libs/middle.jar!/libs/inner.jar!/fixture/Main");
    assert!(!r.partial);
}

#[test]
fn nesting_stops_at_the_depth_limit() {
    let mut jar = fixture("exec-process");
    for _ in 0..5 {
        jar = zip(&[("libs/inner.jar", &jar)], false);
    }
    let r = scan("deep.jar", &jar);
    assert!(r.partial);
    assert!(warning_codes(&r).contains(&"nested.too-deep"));
    assert!(!rules(&r).contains(&"exec.process"));
}

#[test]
fn zip_slip_names_are_flagged_and_still_scanned() {
    let r = scan("slip.jar", &zip(&[("../../evil.yml", b"url: https://evil.example.invalid/x")], false));
    assert_eq!(values(&r, "jar.unsafe-path"), ["../../evil.yml"]);
    let url = r.findings.iter().find(|f| f.rule == "str.url").unwrap();
    assert_eq!(url.evidence[0].location, "evil.yml");
}

#[test]
fn duplicate_entries_are_both_scanned() {
    let r = scan(
        "dup.jar",
        &zip(&[("notes.txt", b"https://first.example.invalid/"), ("notes.txt", b"https://second.example.invalid/")], false),
    );
    assert!(warning_codes(&r).contains(&"zip.duplicate-entry"));
    assert_eq!(values(&r, "str.url"), ["first.example.invalid", "second.example.invalid"]);
}

#[test]
fn prepended_data_is_handled_like_the_jvm() {
    let jar = fixture("exec-process");
    let mut stub = b"#!/bin/sh\nexec java -jar \"$0\" \"$@\"\n".to_vec();
    stub.extend(&jar);
    let shifted = scan("stub.jar", &stub);
    let plain = scan("plain.jar", &jar);
    assert_eq!(rules(&shifted), rules(&plain));
    assert!(shifted.warnings.is_empty(), "{:?}", shifted.warnings);
}

#[test]
fn disguised_files() {
    let class = tiny_class();
    let inner = fixture("exec-process");
    let r = scan(
        "disguised.jar",
        &zip(
            &[
                ("assets/icon.png", &class),
                ("assets/data.png", &inner),
                ("lib/thing.dat", b"\x7fELF\x02\x01\x01\0\0\0\0\0"),
                ("a/Fake.class", b"not a class at all"),
                ("META-INF/native/libok.so", b"\x7fELF\x02\x01\x01\0"),
            ],
            false,
        ),
    );
    assert_eq!(values(&r, "res.type-mismatch"), ["a/Fake.class", "assets/data.png", "assets/icon.png", "lib/thing.dat"]);
    assert!(rules(&r).contains(&"exec.process"), "a jar disguised as a png is still scanned");
    assert_eq!(r.stats.classes, 2);
}

#[test]
fn manifest_agent_attribute() {
    let r = scan("agent.jar", &zip(&[("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\nPremain-Class: a.B\r\n")], false));
    assert!(rules(&r).contains(&"code.instrument"));
}

#[test]
fn hex_payload_in_a_text_resource() {
    let hex: String = tiny_class().iter().map(|b| format!("{b:02x}")).collect();
    let r = scan("hex.jar", &zip(&[("data.txt", hex.as_bytes())], false));
    assert!(rules(&r).contains(&"blob.embedded-code"));
    assert_eq!(r.stats.classes, 1);
}

#[test]
fn reports_are_deterministic() {
    for name in fixture_names() {
        let jar = fixture(&name);
        let a = serde_json::to_string(&scan("x.jar", &jar)).unwrap();
        let b = serde_json::to_string(&scan("x.jar", &jar)).unwrap();
        assert_eq!(a, b, "{name}");
    }
}
