#![allow(dead_code)]

use std::path::PathBuf;

use jarsnitch_core::{Limits, Report};

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/jars")
}

pub fn fixture(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(format!("{name}.jar"));
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "{}: {e} (run fixtures/build.ps1 or build.sh)",
            path.display()
        )
    })
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
    r.findings
        .iter()
        .filter(|f| f.rule == rule)
        .filter_map(|f| f.value.as_deref())
        .collect()
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
    tiny_class_named("a/B")
}

/// The smallest valid class file with the given internal name.
pub fn tiny_class_named(name: &str) -> Vec<u8> {
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 5];
    for (s, utf8_index) in [(name, 1u16), ("java/lang/Object", 3)] {
        c.push(1);
        c.extend((s.len() as u16).to_be_bytes());
        c.extend(s.as_bytes());
        c.push(7);
        c.extend(utf8_index.to_be_bytes());
    }
    c.extend([0, 0x21, 0, 2, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0]);
    c
}

/// An exception handler: start, end and handler offsets, and the class it catches (`None` for any).
pub type Handler<'a> = (u16, u16, u16, Option<&'a str>);

/// A field or method for [`class`]. A method's code is its bytecode plus handlers,
/// `(start, end, handler, catch type)`.
pub struct Member<'a> {
    pub access: u16,
    pub name: &'a str,
    pub desc: &'a str,
    pub code: Option<(&'a [u8], &'a [Handler<'a>])>,
    pub annotation: Option<&'a str>,
}

impl<'a> Member<'a> {
    pub fn method(name: &'a str, code: &'a [u8]) -> Self {
        Member {
            access: 1,
            name,
            desc: "()V",
            code: Some((code, &[])),
            annotation: None,
        }
    }
    pub fn field(access: u16, name: &'a str, desc: &'a str) -> Self {
        Member {
            access,
            name,
            desc,
            code: None,
            annotation: None,
        }
    }
}

#[derive(Default)]
struct Pool {
    bytes: Vec<u8>,
    count: u16,
}

impl Pool {
    fn utf8(&mut self, s: &str) -> [u8; 2] {
        self.bytes.push(1);
        self.bytes.extend((s.len() as u16).to_be_bytes());
        self.bytes.extend(s.as_bytes());
        self.count += 1;
        self.count.to_be_bytes()
    }
    fn class(&mut self, name: &str) -> [u8; 2] {
        let n = self.utf8(name);
        self.bytes.push(7);
        self.bytes.extend(n);
        self.count += 1;
        self.count.to_be_bytes()
    }
    /// A RuntimeVisibleAnnotations attribute holding one annotation without elements.
    fn annotation(&mut self, ty: &str) -> Vec<u8> {
        let mut a = self.utf8("RuntimeVisibleAnnotations").to_vec();
        a.extend(6u32.to_be_bytes());
        a.extend(1u16.to_be_bytes());
        a.extend(self.utf8(ty));
        a.extend(0u16.to_be_bytes());
        a
    }
    fn member(&mut self, m: &Member) -> Vec<u8> {
        let mut attrs = Vec::new();
        let mut n = 0u16;
        if let Some((code, handlers)) = m.code {
            let mut body = vec![0, 10, 0, 10];
            body.extend((code.len() as u32).to_be_bytes());
            body.extend(code);
            body.extend((handlers.len() as u16).to_be_bytes());
            for &(start, end, handler, catch) in handlers {
                for x in [start, end, handler] {
                    body.extend(x.to_be_bytes());
                }
                body.extend(catch.map_or([0, 0], |t| self.class(t)));
            }
            body.extend([0, 0]);
            attrs.extend(self.utf8("Code"));
            attrs.extend((body.len() as u32).to_be_bytes());
            attrs.extend(body);
            n += 1;
        }
        if let Some(ty) = m.annotation {
            attrs.extend(self.annotation(ty));
            n += 1;
        }
        let mut out = m.access.to_be_bytes().to_vec();
        out.extend(self.utf8(m.name));
        out.extend(self.utf8(m.desc));
        out.extend(n.to_be_bytes());
        out.extend(attrs);
        out
    }
}

/// A class file `name extends java/lang/Object` with these fields and methods, annotated
/// with `annotation` if it's set.
pub fn class(
    name: &str,
    fields: &[Member],
    methods: &[Member],
    annotation: Option<&str>,
) -> Vec<u8> {
    let mut p = Pool::default();
    let (this, sup) = (p.class(name), p.class("java/lang/Object"));
    let fields_bytes: Vec<u8> = fields.iter().flat_map(|m| p.member(m)).collect();
    let methods_bytes: Vec<u8> = methods.iter().flat_map(|m| p.member(m)).collect();
    let attr = annotation.map(|ty| p.annotation(ty));
    let mut c = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61];
    c.extend((p.count + 1).to_be_bytes());
    c.extend(&p.bytes);
    c.extend([0, 0x21]);
    c.extend(this);
    c.extend(sup);
    c.extend([0, 0]);
    c.extend((fields.len() as u16).to_be_bytes());
    c.extend(fields_bytes);
    c.extend((methods.len() as u16).to_be_bytes());
    c.extend(methods_bytes);
    c.extend((attr.is_some() as u16).to_be_bytes());
    c.extend(attr.unwrap_or_default());
    c
}
