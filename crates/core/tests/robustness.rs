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
    let jar = zip(
        &[("a.txt", &data), ("b.txt", &data), ("c.txt", &data)],
        false,
    );
    let small = Limits {
        max_total_bytes: 1000,
        ..Limits::default()
    };
    let r = jarsnitch_core::scan("x.jar", &jar, &small);
    assert!(r.partial && r.stats.uncompressed_bytes <= 1000);

    let few = Limits {
        max_entries: 2,
        ..Limits::default()
    };
    let r = jarsnitch_core::scan("x.jar", &jar, &few);
    assert!(r.partial && r.stats.entries <= 2);
}

#[test]
fn nested_jars_are_scanned_with_their_path() {
    let inner = fixture("exec-process");
    let outer = zip(&[("libs/inner.jar", &inner)], true);
    let r = scan("outer.jar", &zip(&[("libs/middle.jar", &outer)], false));
    assert_eq!(
        values(&r, "jar.nested"),
        ["libs/middle.jar", "libs/middle.jar!/libs/inner.jar"]
    );
    let exec = r
        .findings
        .iter()
        .find(|f| f.rule == "exec.process")
        .unwrap();
    assert_eq!(
        exec.evidence[0].location,
        "libs/middle.jar!/libs/inner.jar!/fixture/Main"
    );
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
    let r = scan(
        "slip.jar",
        &zip(
            &[("../../evil.yml", b"url: https://evil.example.invalid/x")],
            false,
        ),
    );
    assert_eq!(values(&r, "jar.unsafe-path"), ["../../evil.yml"]);
    let url = r.findings.iter().find(|f| f.rule == "str.url").unwrap();
    assert_eq!(url.evidence[0].location, "evil.yml");
}

#[test]
fn duplicate_entries_are_both_scanned() {
    let r = scan(
        "dup.jar",
        &zip(
            &[
                ("notes.txt", b"https://first.example.invalid/"),
                ("notes.txt", b"https://second.example.invalid/"),
            ],
            false,
        ),
    );
    assert!(warning_codes(&r).contains(&"zip.duplicate-entry"));
    assert_eq!(
        values(&r, "str.url"),
        ["first.example.invalid", "second.example.invalid"]
    );
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
    assert_eq!(
        values(&r, "res.type-mismatch"),
        [
            "a/Fake.class",
            "assets/data.png",
            "assets/icon.png",
            "lib/thing.dat"
        ]
    );
    assert!(
        rules(&r).contains(&"exec.process"),
        "a jar disguised as a png is still scanned"
    );
    assert_eq!(r.stats.classes, 2);
}

#[test]
fn manifest_agent_attribute() {
    let r = scan(
        "agent.jar",
        &zip(
            &[(
                "META-INF/MANIFEST.MF",
                b"Manifest-Version: 1.0\r\nPremain-Class: a.B\r\n",
            )],
            false,
        ),
    );
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
fn skin_textures_are_not_hidden_text() {
    // {"textures":{"SKIN":{"url":"http://textures.minecraft.net/texture/abc123"}}}
    let skin = "eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90ZXh0dXJlcy5taW5lY3JhZnQubmV0L3RleHR1cmUvYWJjMTIzIn19fQ==";
    let r = scan(
        "heads.jar",
        &zip(
            &[("heads.yml", format!("texture: {skin}\n").as_bytes())],
            false,
        ),
    );
    assert!(
        values(&r, "str.url").is_empty(),
        "{:?}",
        values(&r, "str.url")
    );
    assert!(!rules(&r).contains(&"str.hidden-text"));

    // https://cdn.example.invalid/a.jar
    let hidden = "aHR0cHM6Ly9jZG4uZXhhbXBsZS5pbnZhbGlkL2EuamFy";
    let r = scan(
        "heads.jar",
        &zip(
            &[("heads.yml", format!("texture: {hidden}\n").as_bytes())],
            false,
        ),
    );
    assert_eq!(values(&r, "str.url"), ["cdn.example.invalid"]);
    assert!(rules(&r).contains(&"str.hidden-text"));
}

#[test]
fn domain_lists_are_not_listed_host_by_host() {
    let mut list: String = (0..150)
        .map(|i| {
            format!(
                "site{i}.example.com
"
            )
        })
        .collect();
    list.push_str(
        "// Submitted by https://registry.example.invalid/
",
    );
    let hook = format!(
        "https://discord.com/api/webhooks/000000000000000000/{}
",
        "x".repeat(68)
    );
    list.push_str(&hook);
    let r = scan(
        "psl.jar",
        &zip(
            &[("mozilla/public-suffix-list.txt", list.as_bytes())],
            false,
        ),
    );
    assert!(
        values(&r, "str.url").is_empty(),
        "{:?}",
        values(&r, "str.url")
    );
    assert_eq!(
        values(&r, "str.discord-webhook"),
        ["000000000000000000"],
        "webhooks still count"
    );
    assert_eq!(warning_codes(&r), ["text.host-list"]);
    assert!(!r.partial);
}

/// A class whose Dynamic constant's bootstrap lists another Dynamic constant `n` times,
/// which lists it `n` times again, and so on.
fn self_referential_constants(n: u16) -> Vec<u8> {
    let mut pool: Vec<u8> = Vec::new();
    for entry in [
        &b"\x01\x00\x06p/Blow"[..],
        b"\x07\x00\x01",
        b"\x01\x00\x10java/lang/Object",
        b"\x07\x00\x03", // 1-4
        b"\x01\x00\x04Code",
        b"\x01\x00\x01m",
        b"\x01\x00\x03()V",
        b"\x01\x00\x10BootstrapMethods", // 5-8
        b"\x01\x00\x01x",
        b"\x01\x00\x01I",
        b"\x0c\x00\x09\x00\x0a", // 9-11: x:I
        b"\x01\x00\x03bsm",
        b"\x01\x00\x14()Ljava/lang/Object;",
        b"\x0c\x00\x0c\x00\x0d", // 12-14
        b"\x0a\x00\x02\x00\x0e",
        b"\x0f\x06\x00\x0f", // 15 Methodref, 16 MethodHandle
        b"\x11\x00\x00\x00\x0b",
        b"\x11\x00\x01\x00\x0b", // 17, 18: Dynamic x:I, bootstrap 0 and 1
    ] {
        pool.extend(entry);
    }
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 19];
    c.extend(pool);
    c.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0]);
    // One method, m()V: ldc_w #17, return.
    let code = [0, 10, 0, 10, 0, 0, 0, 4, 0x13, 0, 17, 0xb1, 0, 0, 0, 0];
    c.extend([0, 1, 0, 1, 0, 6, 0, 7, 0, 1, 0, 5]);
    c.extend((code.len() as u32).to_be_bytes());
    c.extend(code);
    let mut bootstrap = vec![0, 2];
    for _ in 0..2 {
        bootstrap.extend([0, 16]);
        bootstrap.extend(n.to_be_bytes());
        bootstrap.extend((0..n).flat_map(|_| [0, 18]));
    }
    c.extend([0, 1, 0, 8]);
    c.extend((bootstrap.len() as u32).to_be_bytes());
    c.extend(bootstrap);
    c
}

/// A class with `n` Dynamic constants that all share one bootstrap method, whose `a`
/// arguments cycle through those constants.
fn shared_bootstrap(n: u16, a: u16) -> Vec<u8> {
    let mut pool: Vec<u8> = Vec::new();
    for entry in [
        &b"\x01\x00\x06p/Wide"[..],
        b"\x07\x00\x01",
        b"\x01\x00\x10java/lang/Object",
        b"\x07\x00\x03", // 1-4
        b"\x01\x00\x04Code",
        b"\x01\x00\x01m",
        b"\x01\x00\x03()V",
        b"\x01\x00\x10BootstrapMethods", // 5-8
        b"\x01\x00\x01x",
        b"\x01\x00\x01I",
        b"\x0c\x00\x09\x00\x0a", // 9-11: x:I
        b"\x01\x00\x03bsm",
        b"\x01\x00\x14()Ljava/lang/Object;",
        b"\x0c\x00\x0c\x00\x0d", // 12-14
        b"\x0a\x00\x02\x00\x0e",
        b"\x0f\x06\x00\x0f", // 15 Methodref, 16 MethodHandle
    ] {
        pool.extend(entry);
    }
    for _ in 0..n {
        pool.extend(b"\x11\x00\x00\x00\x0b"); // 17.. Dynamic x:I, bootstrap 0
    }
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
    c.extend((17 + n).to_be_bytes());
    c.extend(pool);
    c.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0]);
    let code = [0, 10, 0, 10, 0, 0, 0, 4, 0x13, 0, 17, 0xb1, 0, 0, 0, 0];
    c.extend([0, 1, 0, 1, 0, 6, 0, 7, 0, 1, 0, 5]);
    c.extend((code.len() as u32).to_be_bytes());
    c.extend(code);
    let mut bootstrap = vec![0, 1, 0, 16];
    bootstrap.extend(a.to_be_bytes());
    bootstrap.extend((0..a).flat_map(|k| (17 + k % n).to_be_bytes()));
    c.extend([0, 1, 0, 8]);
    c.extend((bootstrap.len() as u32).to_be_bytes());
    c.extend(bootstrap);
    c
}

#[test]
fn constants_sharing_a_bootstrap_method_are_fingerprinted_quickly() {
    // Resolved per constant this was 3 * n * a hashes: about 30 million here.
    let jar = zip(&[("p/Wide.class", &shared_bootstrap(3000, 3000))], false);
    let start = std::time::Instant::now();
    let d = jarsnitch_core::diff(
        "a.jar",
        &jar,
        "b.jar",
        &jar,
        &jarsnitch_core::Limits::default(),
    );
    assert!(
        d.same_code() && start.elapsed().as_secs() < 5,
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn self_referential_constants_are_fingerprinted_quickly() {
    // Without a bound this is n^4 hashes: minutes at 400, years at 65535.
    let jar = zip(&[("p/Blow.class", &self_referential_constants(400))], false);
    let start = std::time::Instant::now();
    let d = jarsnitch_core::diff(
        "a.jar",
        &jar,
        "b.jar",
        &jar,
        &jarsnitch_core::Limits::default(),
    );
    assert!(
        d.same_code() && start.elapsed().as_secs() < 5,
        "{:?}",
        start.elapsed()
    );
}

/// A class whose method `m` names `len`-byte strings `n` times each: an invokestatic of
/// `ooo.nnn`, an ldc of `sss`, and an invokedynamic whose bootstrap method lists a handle to
/// `ooo.nnn` `a` times.
fn repetitive(len: u16, n: u16, a: u16) -> Vec<u8> {
    let utf8 = |s: &[u8]| [&[1u8][..], &(s.len() as u16).to_be_bytes(), s].concat();
    let long = |c: u8| utf8(&vec![c; usize::from(len)]);
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 18];
    for entry in [
        utf8(b"p/Rep"),
        vec![7, 0, 1],
        utf8(b"java/lang/Object"),
        vec![7, 0, 3], // 1-4
        utf8(b"Code"),
        utf8(b"m"),
        utf8(b"()V"),
        utf8(b"BootstrapMethods"), // 5-8
        long(b'o'),
        vec![7, 0, 9],
        long(b'n'),
        vec![12, 0, 11, 0, 7],
        vec![10, 0, 10, 0, 12], // 9-13: ooo.nnn()V
        long(b's'),
        vec![8, 0, 14],
        vec![15, 6, 0, 13],
        vec![18, 0, 0, 0, 12], // 14 sss, 15 String, 16 handle, 17 indy
    ] {
        c.extend(entry);
    }
    c.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0]);
    let mut ops = Vec::new();
    for _ in 0..n {
        ops.extend([0xb8, 0, 13, 0x13, 0, 15, 0xba, 0, 17, 0, 0]);
    }
    ops.push(0xb1);
    let mut code = vec![0, 10, 0, 10];
    code.extend((ops.len() as u32).to_be_bytes());
    code.extend(ops);
    code.extend([0, 0, 0, 0]);
    c.extend([0, 1, 0, 9, 0, 6, 0, 7, 0, 1, 0, 5]);
    c.extend((code.len() as u32).to_be_bytes());
    c.extend(code);
    let mut bootstrap = vec![0, 1, 0, 16];
    bootstrap.extend(a.to_be_bytes());
    bootstrap.extend((0..a).flat_map(|_| [0, 16]));
    c.extend([0, 1, 0, 8]);
    c.extend((bootstrap.len() as u32).to_be_bytes());
    c.extend(bootstrap);
    c
}

#[test]
fn repeated_invokedynamic_is_scanned_quickly() {
    // Walking the bootstrap arguments at every invokedynamic was 65 million calls here.
    let jar = zip(&[("p/Rep.class", &repetitive(1, 1000, 65535))], false);
    let start = std::time::Instant::now();
    let r = scan("x.jar", &jar);
    assert!(
        !r.partial && start.elapsed().as_secs() < 5,
        "{:?} {:?}",
        start.elapsed(),
        warning_codes(&r)
    );
}

#[test]
fn long_strings_named_over_and_over_stop_at_the_code_budget() {
    // 5,000 instructions each naming 120 KB of strings: 1.2 GB to walk, and to hash before
    // each string was hashed once per class.
    let jar = zip(&[("p/Rep.class", &repetitive(60_000, 5000, 1000))], false);
    let limits = Limits {
        max_code_work: 16 << 20,
        ..Limits::default()
    };
    let start = std::time::Instant::now();
    let r = jarsnitch_core::scan("x.jar", &jar, &limits);
    assert!(
        r.partial && warning_codes(&r).contains(&"class.limit"),
        "{:?}",
        warning_codes(&r)
    );
    let d = jarsnitch_core::diff("a.jar", &jar, "b.jar", &jar, &limits);
    assert_eq!(d.code.unchanged_classes, 1);
    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
}

#[test]
fn class_order_doesnt_change_fingerprints() {
    let (a, b) = (shared_bootstrap(300, 3000), self_referential_constants(40));
    let ab = zip(&[("p/Wide.class", &a), ("p/Blow.class", &b)], false);
    let ba = zip(&[("p/Blow.class", &b), ("p/Wide.class", &a)], false);
    let d = jarsnitch_core::diff("ab.jar", &ab, "ba.jar", &ba, &Limits::default());
    assert!(d.same_code(), "{:?}", d.code.changed_classes);
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
