#![allow(dead_code)]

use std::path::PathBuf;

use jarsnitch_core::{Limits, Report};

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/jars")
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(format!("{name}.jar"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e} (run fixtures/build.ps1 or build.sh)", path.display()))
}

pub fn fixture_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(fixtures_dir())
        .expect("fixtures/jars")
        .filter_map(|e| {
            let name = e.ok()?.file_name().into_string().ok()?;
            Some(name.strip_suffix(".jar")?.to_string())
        })
        .collect();
    names.sort();
    names
}

pub fn scan(name: &str, bytes: &[u8]) -> Report {
    jarsnitch_core::scan(name, bytes, &Limits::default())
}

pub fn rules(r: &Report) -> Vec<&str> {
    r.findings.iter().map(|f| f.rule.as_str()).collect()
}

pub fn values<'a>(r: &'a Report, rule: &str) -> Vec<&'a str> {
    r.findings.iter().filter(|f| f.rule == rule).filter_map(|f| f.value.as_deref()).collect()
}

/// A zip archive with the given entries, stored or deflated. CRCs are left at zero:
/// jarsnitch doesn't check them.
pub fn zip(entries: &[(&str, &[u8])], deflate: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let (method, body) = if deflate {
            (8u16, miniz_oxide::deflate::compress_to_vec(data, 6))
        } else {
            (0u16, data.to_vec())
        };
        let offset = out.len() as u32;
        let header = |sig: u32, central: bool| {
            let mut h = sig.to_le_bytes().to_vec();
            if central {
                h.extend(20u16.to_le_bytes()); // version made by
            }
            h.extend(20u16.to_le_bytes()); // version needed
            h.extend(0u16.to_le_bytes()); // flags
            h.extend(method.to_le_bytes());
            h.extend([0u8; 4]); // time, date
            h.extend(0u32.to_le_bytes()); // crc
            h.extend((body.len() as u32).to_le_bytes());
            h.extend((data.len() as u32).to_le_bytes());
            h.extend((name.len() as u16).to_le_bytes());
            h.extend(0u16.to_le_bytes()); // extra length
            if central {
                h.extend([0u8; 6]); // comment length, disk, internal attributes
                h.extend(0u32.to_le_bytes()); // external attributes
                h.extend(offset.to_le_bytes());
            }
            h.extend(name.as_bytes());
            h
        };
        out.extend(header(0x0403_4b50, false));
        out.extend(&body);
        central.extend(header(0x0201_4b50, true));
    }
    let cd_offset = out.len() as u32;
    out.extend(&central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    out.extend([0u8; 4]); // disk numbers
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    out.extend((central.len() as u32).to_le_bytes());
    out.extend(cd_offset.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

/// The smallest valid class file: `a/B extends java/lang/Object`, no members.
pub fn tiny_class() -> Vec<u8> {
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 5];
    for s in ["a/B", "java/lang/Object"] {
        c.push(1);
        c.extend((s.len() as u16).to_be_bytes());
        c.extend(s.as_bytes());
        let utf8_index = if s == "a/B" { 1u16 } else { 3 };
        c.push(7);
        c.extend(utf8_index.to_be_bytes());
    }
    c.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
    c
}
