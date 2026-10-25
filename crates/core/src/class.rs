//! Minimal class-file parser. Reads only what the rules need (constant pool, names,
//! method `Code`, `BootstrapMethods`) and skips every other attribute, so junk
//! attributes from obfuscators can't make a class unreadable.

use std::borrow::Cow;

pub enum Const<'a> {
    Utf8(Cow<'a, str>),
    Class(u16),
    Str(u16),
    /// Field, method or interface-method reference: class index, name-and-type index.
    Ref(u16, u16),
    NameType(u16, u16),
    /// Reference kind and reference index.
    Handle(u8, u16),
    /// Bootstrap method index.
    Indy(u16),
    Other,
}

pub struct Method<'a> {
    pub name: u16,
    pub desc: u16,
    pub code: Option<&'a [u8]>,
}

pub struct ClassFile<'a> {
    pub cp: Vec<Const<'a>>,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<u16>,
    pub methods: Vec<Method<'a>>,
    /// Method handle index and argument indices for each bootstrap method.
    pub bootstrap: Vec<(u16, Vec<u16>)>,
}

/// True for class-file bytes. `CAFEBABE` is also the magic of a Mach-O universal
/// binary; there bytes 4..8 hold a small architecture count instead of a version >= 45.
pub fn is_class(b: &[u8]) -> bool {
    b.len() >= 10 && b[..4] == [0xCA, 0xFE, 0xBA, 0xBE] && u16::from_be_bytes([b[6], b[7]]) >= 45
}

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

type R<T> = Result<T, &'static str>;

impl<'a> Reader<'a> {
    fn bytes(&mut self, n: usize) -> R<&'a [u8]> {
        let s = self.b.get(self.p..self.p.checked_add(n).ok_or("overflow")?).ok_or("truncated")?;
        self.p += n;
        Ok(s)
    }
    fn u1(&mut self) -> R<u8> {
        Ok(self.bytes(1)?[0])
    }
    fn u2(&mut self) -> R<u16> {
        let s = self.bytes(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u4(&mut self) -> R<u32> {
        let s = self.bytes(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn attributes(&mut self) -> R<Vec<(u16, &'a [u8])>> {
        let n = self.u2()?;
        let mut out = Vec::with_capacity(usize::from(n.min(64)));
        for _ in 0..n {
            let name = self.u2()?;
            let len = self.u4()? as usize;
            out.push((name, self.bytes(len)?));
        }
        Ok(out)
    }
}

impl<'a> ClassFile<'a> {
    pub fn parse(b: &'a [u8]) -> R<Self> {
        let mut r = Reader { b, p: 8 };
        if !is_class(b) {
            return Err("not a class file");
        }
        let cp_count = r.u2()?;
        let mut cp = Vec::with_capacity(usize::from(cp_count));
        cp.push(Const::Other);
        while cp.len() < usize::from(cp_count) {
            let tag = r.u1()?;
            let c = match tag {
                1 => {
                    let len = usize::from(r.u2()?);
                    let raw = r.bytes(len)?;
                    // Modified UTF-8 differs from UTF-8 only for NUL and supplementary
                    // characters; a lossy decode is fine for matching and display.
                    Const::Utf8(match std::str::from_utf8(raw) {
                        Ok(s) => Cow::Borrowed(s),
                        Err(_) => Cow::Owned(String::from_utf8_lossy(raw).into_owned()),
                    })
                }
                3 | 4 => {
                    r.bytes(4)?;
                    Const::Other
                }
                5 | 6 => {
                    r.bytes(8)?;
                    cp.push(Const::Other);
                    Const::Other
                }
                7 => Const::Class(r.u2()?),
                8 => Const::Str(r.u2()?),
                9..=11 => Const::Ref(r.u2()?, r.u2()?),
                12 => Const::NameType(r.u2()?, r.u2()?),
                15 => Const::Handle(r.u1()?, r.u2()?),
                16 | 19 | 20 => {
                    r.u2()?;
                    Const::Other
                }
                17 => {
                    r.bytes(4)?;
                    Const::Other
                }
                18 => {
                    let bsm = r.u2()?;
                    r.u2()?;
                    Const::Indy(bsm)
                }
                _ => return Err("bad constant pool tag"),
            };
            cp.push(c);
        }
        r.u2()?; // access flags
        let this_class = r.u2()?;
        let super_class = r.u2()?;
        let n = r.u2()?;
        let interfaces = (0..n).map(|_| r.u2()).collect::<R<Vec<_>>>()?;

        let mut fields = Vec::new();
        for _ in 0..r.u2()? {
            r.u2()?;
            fields.push(r.u2()?);
            r.u2()?;
            r.attributes()?;
        }

        let mut cf = ClassFile { cp, this_class, super_class, interfaces, fields, methods: Vec::new(), bootstrap: Vec::new() };
        for _ in 0..r.u2()? {
            r.u2()?;
            let name = r.u2()?;
            let desc = r.u2()?;
            let code = r
                .attributes()?
                .into_iter()
                .find(|(n, _)| cf.utf8(*n) == Some("Code"))
                .and_then(|(_, body)| {
                    let len = u32::from_be_bytes(body.get(4..8)?.try_into().ok()?) as usize;
                    body.get(8..8usize.checked_add(len)?)
                });
            cf.methods.push(Method { name, desc, code });
        }
        for (name, body) in r.attributes()? {
            if cf.utf8(name) == Some("BootstrapMethods") {
                cf.bootstrap = parse_bootstrap(body).unwrap_or_default();
            }
        }
        Ok(cf)
    }

    pub fn utf8(&self, i: u16) -> Option<&str> {
        match self.cp.get(usize::from(i))? {
            Const::Utf8(s) => Some(s),
            _ => None,
        }
    }

    pub fn class_name(&self, i: u16) -> Option<&str> {
        match self.cp.get(usize::from(i))? {
            Const::Class(n) => self.utf8(*n),
            _ => None,
        }
    }

    pub fn string(&self, i: u16) -> Option<&str> {
        match self.cp.get(usize::from(i))? {
            Const::Str(n) => self.utf8(*n),
            _ => None,
        }
    }

    /// Owner, name and descriptor of a field or method reference.
    pub fn member(&self, i: u16) -> Option<(&str, &str, &str)> {
        let Const::Ref(class, nat) = self.cp.get(usize::from(i))? else { return None };
        let Const::NameType(name, desc) = self.cp.get(usize::from(*nat))? else { return None };
        Some((self.class_name(*class)?, self.utf8(*name)?, self.utf8(*desc)?))
    }

    /// The method a method-handle constant points at. Field handles (kinds 1-4) are ignored.
    pub fn handle(&self, i: u16) -> Option<(&str, &str, &str)> {
        match self.cp.get(usize::from(i))? {
            Const::Handle(kind, r) if (5..=9).contains(kind) => self.member(*r),
            _ => None,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.class_name(self.this_class)
    }

    pub fn super_name(&self) -> Option<&str> {
        self.class_name(self.super_class)
    }
}

fn parse_bootstrap(body: &[u8]) -> R<Vec<(u16, Vec<u16>)>> {
    let mut r = Reader { b: body, p: 0 };
    let n = r.u2()?;
    let mut out = Vec::with_capacity(usize::from(n.min(1024)));
    for _ in 0..n {
        let mh = r.u2()?;
        let argc = r.u2()?;
        out.push((mh, (0..argc).map(|_| r.u2()).collect::<R<Vec<_>>>()?));
    }
    Ok(out)
}

/// The instructions the rules care about.
pub enum Op {
    /// invokevirtual, invokespecial, invokestatic or invokeinterface on a constant-pool reference.
    Invoke(u16),
    /// invokedynamic on an `InvokeDynamic` constant.
    Indy(u16),
    /// ldc, ldc_w or ldc2_w of a constant.
    Ldc(u16),
}

/// Walks bytecode and yields interesting instructions. Stops quietly at the first
/// malformed instruction; what was read so far still counts.
pub struct Ops<'a> {
    code: &'a [u8],
    pc: usize,
}

pub fn ops(code: &[u8]) -> Ops<'_> {
    Ops { code, pc: 0 }
}

impl Ops<'_> {
    fn u2(&self, at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([*self.code.get(at)?, *self.code.get(at + 1)?]))
    }
    fn i4(&self, at: usize) -> Option<i32> {
        Some(i32::from_be_bytes(self.code.get(at..at + 4)?.try_into().ok()?))
    }
    /// Length of the instruction at `pc`, or None if it's invalid.
    fn len(&self, pc: usize) -> Option<usize> {
        let op = *self.code.get(pc)?;
        Some(match op {
            0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
            0x11 | 0x13 | 0x14 | 0x84 | 0x99..=0xa8 | 0xb2..=0xb8 | 0xbb | 0xbd | 0xc0 | 0xc1 | 0xc6 | 0xc7 => 3,
            0xc5 => 4,
            0xb9 | 0xba | 0xc8 | 0xc9 => 5,
            0xc4 => {
                if *self.code.get(pc + 1)? == 0x84 {
                    6
                } else {
                    4
                }
            }
            0xaa | 0xab => {
                let base = pc + 1 + (4 - (pc + 1) % 4) % 4;
                let n = if op == 0xaa {
                    let low = self.i4(base + 4)?;
                    let high = self.i4(base + 8)?;
                    let count = i64::from(high) - i64::from(low) + 1;
                    if !(0..=65536).contains(&count) {
                        return None;
                    }
                    12 + 4 * count as usize
                } else {
                    let pairs = self.i4(base + 4)?;
                    if !(0..=65536).contains(&pairs) {
                        return None;
                    }
                    8 + 8 * pairs as usize
                };
                base - pc + n
            }
            0x00..=0xc3 | 0xca | 0xfe | 0xff => 1,
            _ => return None,
        })
    }
}

impl Iterator for Ops<'_> {
    type Item = Op;

    fn next(&mut self) -> Option<Op> {
        loop {
            let pc = self.pc;
            let len = self.len(pc)?;
            if pc + len > self.code.len() {
                return None;
            }
            self.pc += len;
            let op = match self.code[pc] {
                0xb6..=0xb9 => Op::Invoke(self.u2(pc + 1)?),
                0xba => Op::Indy(self.u2(pc + 1)?),
                0x12 => Op::Ldc(u16::from(self.code[pc + 1])),
                0x13 | 0x14 => Op::Ldc(self.u2(pc + 1)?),
                _ => continue,
            };
            return Some(op);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncated_classes_never_panic() {
        let mut b = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 5, 1, 0, 3, b'a', b'b', b'c'];
        for n in 0..b.len() {
            let _ = ClassFile::parse(&b[..n]);
        }
        b.extend_from_slice(&[0xff; 32]);
        assert!(ClassFile::parse(&b).is_err());
    }

    #[test]
    fn mach_o_universal_is_not_a_class() {
        assert!(!is_class(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 2, 0, 0]));
        assert!(is_class(&[0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 0]));
    }

    #[test]
    fn walker_handles_switch_padding_and_bad_ops() {
        // nop; tableswitch with 2 padding bytes, default, low 0, high 0, one offset; ldc #1
        let mut code = vec![0x00, 0xaa, 0, 0];
        code.extend([0; 16]);
        code.extend([0x12, 1]);
        assert!(matches!(ops(&code).collect::<Vec<_>>()[..], [Op::Ldc(1)]));
        assert_eq!(ops(&[0xcb, 0x12, 1]).count(), 0);
    }
}
