//! Minimal class-file parser. Reads only what the rules need (constant pool, names,
//! method `Code`, `BootstrapMethods`) and skips every other attribute, so junk
//! attributes from obfuscators can't make a class unreadable.

use std::borrow::Cow;
use std::collections::BTreeSet;

use sha2::{Digest, Sha256};

pub enum Const<'a> {
    Utf8(Cow<'a, str>),
    Class(u16),
    Str(u16),
    /// Field, method or interface-method reference: class index, name-and-type index.
    Ref(u16, u16),
    NameType(u16, u16),
    /// Reference kind and reference index.
    Handle(u8, u16),
    /// Bootstrap method index and name-and-type index.
    Indy(u16, u16),
    /// A dynamically computed constant: bootstrap method index and name-and-type index.
    Dynamic(u16, u16),
    /// Descriptor index.
    MethodType(u16),
    /// Integer, float, long or double: the tag and the raw bytes.
    Num(u8, &'a [u8]),
    Other,
}

pub struct Field<'a> {
    pub access: u16,
    pub name: u16,
    pub desc: u16,
    /// Attribute name index and body.
    pub attrs: Vec<(u16, &'a [u8])>,
}

pub struct Method<'a> {
    pub access: u16,
    pub name: u16,
    pub desc: u16,
    pub code: Option<&'a [u8]>,
    /// What follows the bytecode in the `Code` attribute: the exception table first.
    pub after_code: &'a [u8],
    /// Has a `RuntimeVisibleAnnotations` attribute.
    pub annotated: bool,
    /// Attribute name index and body.
    pub attrs: Vec<(u16, &'a [u8])>,
}

/// Attributes whose content changes what the server does with the code: an
/// `@EventHandler` registers a method, a permission annotation guards a command.
const ANNOTATIONS: [&str; 3] = [
    "RuntimeVisibleAnnotations",
    "RuntimeVisibleParameterAnnotations",
    "AnnotationDefault",
];

/// What one class's fingerprints share, so constants that repeat a string or a bootstrap
/// method can't multiply the work: resolved bootstrap methods by BootstrapMethods index and
/// depth, digests of long strings by index, and the calls already listed. The work for a
/// class is then a small multiple of its size, and a print depends only on the class's bytes.
#[derive(Default)]
pub struct Memo {
    done: std::collections::HashMap<(u16, u8), [u8; 32]>,
    long: std::collections::HashMap<u16, [u8; 32]>,
    called: std::collections::HashSet<(u16, u16)>,
}

pub const ACC_PUBLIC: u16 = 0x0001;
pub const ACC_PROTECTED: u16 = 0x0004;
pub const ACC_STATIC: u16 = 0x0008;

pub struct ClassFile<'a> {
    pub cp: Vec<Const<'a>>,
    pub access: u16,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces: Vec<u16>,
    pub fields: Vec<Field<'a>>,
    pub methods: Vec<Method<'a>>,
    /// Method handle index and argument indices for each bootstrap method.
    pub bootstrap: Vec<(u16, Vec<u16>)>,
    /// The class's own attributes: name index and body.
    pub attrs: Vec<(u16, &'a [u8])>,
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
        let s = self
            .b
            .get(self.p..self.p.checked_add(n).ok_or("overflow")?)
            .ok_or("truncated")?;
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
                3 | 4 => Const::Num(tag, r.bytes(4)?),
                5 | 6 => {
                    let b = r.bytes(8)?;
                    cp.push(Const::Num(tag, b));
                    // The slot after a long or double is unusable.
                    Const::Other
                }
                7 => Const::Class(r.u2()?),
                8 => Const::Str(r.u2()?),
                9..=11 => Const::Ref(r.u2()?, r.u2()?),
                12 => Const::NameType(r.u2()?, r.u2()?),
                15 => Const::Handle(r.u1()?, r.u2()?),
                16 => Const::MethodType(r.u2()?),
                19 | 20 => {
                    r.u2()?;
                    Const::Other
                }
                17 => Const::Dynamic(r.u2()?, r.u2()?),
                18 => Const::Indy(r.u2()?, r.u2()?),
                _ => return Err("bad constant pool tag"),
            };
            cp.push(c);
        }
        let access = r.u2()?;
        let this_class = r.u2()?;
        let super_class = r.u2()?;
        let n = r.u2()?;
        let interfaces = (0..n).map(|_| r.u2()).collect::<R<Vec<_>>>()?;

        let mut fields = Vec::new();
        for _ in 0..r.u2()? {
            fields.push(Field {
                access: r.u2()?,
                name: r.u2()?,
                desc: r.u2()?,
                attrs: r.attributes()?,
            });
        }

        let mut cf = ClassFile {
            cp,
            access,
            this_class,
            super_class,
            interfaces,
            fields,
            methods: Vec::new(),
            bootstrap: Vec::new(),
            attrs: Vec::new(),
        };
        for _ in 0..r.u2()? {
            let access = r.u2()?;
            let name = r.u2()?;
            let desc = r.u2()?;
            let attrs = r.attributes()?;
            let annotated = attrs
                .iter()
                .any(|(n, _)| cf.utf8(*n) == Some("RuntimeVisibleAnnotations"));
            let (code, after_code) = attrs
                .iter()
                .find(|(n, _)| cf.utf8(*n) == Some("Code"))
                .and_then(|(_, body)| {
                    let len = u32::from_be_bytes(body.get(4..8)?.try_into().ok()?) as usize;
                    let end = 8usize.checked_add(len)?;
                    Some((body.get(8..end)?, body.get(end..)?))
                })
                .map_or((None, &[][..]), |(c, rest)| (Some(c), rest));
            cf.methods.push(Method {
                access,
                name,
                desc,
                code,
                after_code,
                annotated,
                attrs,
            });
        }
        cf.attrs = r.attributes()?;
        if let Some((_, body)) = cf
            .attrs
            .iter()
            .find(|(n, _)| cf.utf8(*n) == Some("BootstrapMethods"))
        {
            cf.bootstrap = parse_bootstrap(body).unwrap_or_default();
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

    /// The `Utf8` indexes of a field or method reference's owner, name and descriptor.
    fn member_index(&self, i: u16) -> Option<[u16; 3]> {
        let Const::Ref(class, nat) = self.cp.get(usize::from(i))? else {
            return None;
        };
        let Const::NameType(name, desc) = self.cp.get(usize::from(*nat))? else {
            return None;
        };
        let Const::Class(owner) = self.cp.get(usize::from(*class))? else {
            return None;
        };
        let index = [*owner, *name, *desc];
        index
            .iter()
            .all(|&k| self.utf8(k).is_some())
            .then_some(index)
    }

    /// Owner, name and descriptor of a field or method reference.
    pub fn member(&self, i: u16) -> Option<(&str, &str, &str)> {
        let Const::Ref(class, nat) = self.cp.get(usize::from(i))? else {
            return None;
        };
        let Const::NameType(name, desc) = self.cp.get(usize::from(*nat))? else {
            return None;
        };
        Some((
            self.class_name(*class)?,
            self.utf8(*name)?,
            self.utf8(*desc)?,
        ))
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

    /// Access flags, superclass, interfaces and annotations, for comparing two versions
    /// of a class.
    pub fn header_print(&self, memo: &mut Memo) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.access.to_be_bytes());
        self.put_class(&mut h, self.super_class, memo);
        for &i in &self.interfaces {
            self.put_class(&mut h, i, memo);
        }
        self.put_annotations(&mut h, &self.attrs, memo);
        h.finalize().into()
    }

    /// Access flags, constant value and annotations of a field.
    pub fn field_print(
        &self,
        f: &Field,
        calls: &mut BTreeSet<String>,
        memo: &mut Memo,
    ) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(f.access.to_be_bytes());
        if let Some((_, b)) = f
            .attrs
            .iter()
            .find(|(n, _)| self.utf8(*n) == Some("ConstantValue"))
        {
            match b.get(..2) {
                Some(i) => self.put_const(
                    &mut h,
                    b'v',
                    u16::from_be_bytes([i[0], i[1]]),
                    calls,
                    memo,
                    0,
                ),
                None => h.update(b),
            }
        }
        self.put_annotations(&mut h, &f.attrs, memo);
        h.finalize().into()
    }

    /// Fingerprint of a method: its access flags, annotations, code and exception
    /// handlers, with constant-pool indexes replaced by what they point at and branch
    /// offsets by instruction numbers. A recompile or a class rewriter that only
    /// renumbers the constant pool keeps it; any change to an instruction or to a
    /// constant it uses changes it. The methods the code calls go into `calls` as
    /// `owner.name`. Use one `memo` per class.
    pub fn method_print(
        &self,
        m: &Method,
        calls: &mut BTreeSet<String>,
        memo: &mut Memo,
    ) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(m.access.to_be_bytes());
        self.put_annotations(&mut h, &m.attrs, memo);
        let Some(code) = m.code else {
            return h.finalize().into();
        };
        h.update(b"code");
        let walker = ops(code);
        let mut starts = Vec::new();
        let mut end = 0;
        while let Some(n) = walker.len(end).filter(|n| end + n <= code.len()) {
            starts.push(end);
            end += n;
        }
        let u2 = |at: usize| u16::from_be_bytes([code[at], code[at + 1]]);
        let i4 =
            |at: usize| i32::from_be_bytes([code[at], code[at + 1], code[at + 2], code[at + 3]]);
        // Branch targets as instruction numbers, so a longer or shorter instruction
        // elsewhere (ldc becoming ldc_w) doesn't shift them.
        let target = |from: usize, offset: i32| -> [u8; 4] {
            let n = usize::try_from(from as i64 + i64::from(offset))
                .ok()
                .and_then(|t| starts.binary_search(&t).ok());
            n.map_or(u32::MAX, |n| n as u32).to_be_bytes()
        };
        for &pc in &starts {
            let op = code[pc];
            match op {
                // ldc, ldc_w and ldc2_w load a constant; which one depends on its index.
                0x12 => self.put_const(&mut h, b'L', u16::from(code[pc + 1]), calls, memo, 0),
                0x13 | 0x14 => self.put_const(&mut h, b'L', u2(pc + 1), calls, memo, 0),
                0xb2..=0xb9 => {
                    h.update([op]);
                    if !self.put_member(&mut h, u2(pc + 1), op >= 0xb6, calls, memo) {
                        h.update(&code[pc + 1..pc + 3]);
                    }
                }
                0xba | 0xbb | 0xbd | 0xc0 | 0xc1 => {
                    self.put_const(&mut h, op, u2(pc + 1), calls, memo, 0)
                }
                0xc5 => {
                    self.put_const(&mut h, op, u2(pc + 1), calls, memo, 0);
                    h.update([code[pc + 3]]);
                }
                0x99..=0xa8 | 0xc6 | 0xc7 => {
                    h.update([op]);
                    h.update(target(pc, i32::from(u2(pc + 1) as i16)));
                }
                // goto_w and jsr_w are goto and jsr with a longer offset.
                0xc8 | 0xc9 => {
                    h.update([op - 0x21]);
                    h.update(target(pc, i4(pc + 1)));
                }
                0xaa | 0xab => {
                    h.update([op]);
                    let base = pc + 1 + (4 - (pc + 1) % 4) % 4;
                    h.update(target(pc, i4(base)));
                    if op == 0xaa {
                        let (low, high) = (i4(base + 4), i4(base + 8));
                        h.update(low.to_be_bytes());
                        h.update(high.to_be_bytes());
                        for k in 0..(i64::from(high) - i64::from(low) + 1) as usize {
                            h.update(target(pc, i4(base + 12 + 4 * k)));
                        }
                    } else {
                        for k in 0..i4(base + 4) as usize {
                            h.update(i4(base + 8 + 8 * k).to_be_bytes());
                            h.update(target(pc, i4(base + 12 + 8 * k)));
                        }
                    }
                }
                _ => h.update(&code[pc..pc + walker.len(pc).unwrap_or(1)]),
            }
        }
        // Bytes the walker couldn't read count as they are.
        h.update([0xff]);
        h.update(&code[end..]);
        // Exception handlers decide which errors are caught and where the code goes next:
        // the instructions each covers, its handler and the class it catches.
        h.update(b"handlers");
        let at = |pc: u16| -> [u8; 4] {
            let pc = usize::from(pc);
            let n = if pc == code.len() {
                Some(starts.len())
            } else {
                starts.binary_search(&pc).ok()
            };
            n.map_or(u32::MAX, |n| n as u32).to_be_bytes()
        };
        let t = m.after_code;
        let n = t
            .get(..2)
            .map_or(0, |b| usize::from(u16::from_be_bytes([b[0], b[1]])));
        match t.get(2..2 + 8 * n) {
            Some(table) => {
                for e in table.as_chunks::<8>().0 {
                    let u = |i: usize| u16::from_be_bytes([e[i], e[i + 1]]);
                    for pc in [u(0), u(2), u(4)] {
                        h.update(at(pc));
                    }
                    match u(6) {
                        0 => h.update(b"*"),
                        c => self.put_class(&mut h, c, memo),
                    }
                }
            }
            None => h.update(t),
        }
        h.finalize().into()
    }

    /// Annotations the server or a framework acts on, by value.
    fn put_annotations(&self, h: &mut Sha256, attrs: &[(u16, &[u8])], memo: &mut Memo) {
        for (name, body) in attrs {
            let Some(kind) = self.utf8(*name).filter(|n| ANNOTATIONS.contains(n)) else {
                continue;
            };
            put(h, kind);
            let mut r = Reader { b: body, p: 0 };
            let read = match kind {
                "RuntimeVisibleParameterAnnotations" => {
                    self.put_parameter_annotations(h, &mut r, memo)
                }
                "AnnotationDefault" => self.put_element(h, &mut r, 0, memo),
                _ => self.put_annotation_list(h, &mut r, memo),
            };
            // Unreadable or extra bytes count as they are.
            if read.is_err() || r.p != body.len() {
                h.update([0xff]);
                h.update(body);
            }
        }
    }

    fn put_parameter_annotations(&self, h: &mut Sha256, r: &mut Reader, memo: &mut Memo) -> R<()> {
        let n = r.u1()?;
        h.update([n]);
        for _ in 0..n {
            self.put_annotation_list(h, r, memo)?;
        }
        Ok(())
    }

    fn put_annotation_list(&self, h: &mut Sha256, r: &mut Reader, memo: &mut Memo) -> R<()> {
        let n = r.u2()?;
        h.update(n.to_be_bytes());
        for _ in 0..n {
            self.put_annotation(h, r, 0, memo)?;
        }
        Ok(())
    }

    fn put_annotation(&self, h: &mut Sha256, r: &mut Reader, depth: u8, memo: &mut Memo) -> R<()> {
        self.put_utf8(h, r.u2()?, memo);
        let n = r.u2()?;
        h.update(n.to_be_bytes());
        for _ in 0..n {
            self.put_utf8(h, r.u2()?, memo);
            self.put_element(h, r, depth, memo)?;
        }
        Ok(())
    }

    fn put_element(&self, h: &mut Sha256, r: &mut Reader, depth: u8, memo: &mut Memo) -> R<()> {
        if depth > 32 {
            return Err("annotation nested too deep");
        }
        let tag = r.u1()?;
        h.update([tag]);
        match tag {
            b's' | b'c' => self.put_utf8(h, r.u2()?, memo),
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' => {
                let i = r.u2()?;
                match self.cp.get(usize::from(i)) {
                    Some(Const::Num(t, b)) => {
                        h.update([*t]);
                        h.update(b);
                    }
                    _ => h.update([b'?', (i >> 8) as u8, i as u8]),
                }
            }
            b'e' => {
                self.put_utf8(h, r.u2()?, memo);
                self.put_utf8(h, r.u2()?, memo);
            }
            b'@' => self.put_annotation(h, r, depth + 1, memo)?,
            b'[' => {
                let n = r.u2()?;
                h.update(n.to_be_bytes());
                for _ in 0..n {
                    self.put_element(h, r, depth + 1, memo)?;
                }
            }
            _ => return Err("bad annotation element"),
        }
        Ok(())
    }

    fn put_class(&self, h: &mut Sha256, i: u16, memo: &mut Memo) {
        match self.cp.get(usize::from(i)) {
            Some(Const::Class(n)) if self.utf8(*n).is_some() => self.put_utf8(h, *n, memo),
            _ => h.update([b'?', (i >> 8) as u8, i as u8]),
        }
    }

    /// A string by value. A long one goes in as its digest, worked out once per class, so
    /// code that names it many times costs no more than code that names a short one.
    fn put_utf8(&self, h: &mut Sha256, i: u16, memo: &mut Memo) {
        match self.utf8(i) {
            Some(s) if s.len() <= 64 => put(h, s),
            Some(s) => {
                let d = *memo
                    .long
                    .entry(i)
                    .or_insert_with(|| Sha256::digest(s.as_bytes()).into());
                h.update(u32::MAX.to_be_bytes());
                h.update(d);
            }
            None => h.update([b'?', (i >> 8) as u8, i as u8]),
        }
    }

    /// A field or method reference by value, and, when `call`, its method into `calls`;
    /// false when `i` isn't a readable reference.
    fn put_member(
        &self,
        h: &mut Sha256,
        i: u16,
        call: bool,
        calls: &mut BTreeSet<String>,
        memo: &mut Memo,
    ) -> bool {
        let Some(index @ [owner, name, _]) = self.member_index(i) else {
            return false;
        };
        for k in index {
            self.put_utf8(h, k, memo);
        }
        if call && memo.called.insert((owner, name)) {
            calls.insert(format!(
                "{}.{}",
                self.utf8(owner).unwrap_or_default(),
                self.utf8(name).unwrap_or_default()
            ));
        }
        true
    }

    /// A tag byte, then the constant at `i` by value. Anything it can't resolve (a bootstrap
    /// method nested more than 4 deep, a broken reference) counts by index, so the same
    /// bytes always print the same.
    fn put_const(
        &self,
        h: &mut Sha256,
        tag: u8,
        i: u16,
        calls: &mut BTreeSet<String>,
        memo: &mut Memo,
        depth: u8,
    ) {
        h.update([tag]);
        match self.cp.get(usize::from(i)) {
            Some(Const::Str(n)) => {
                h.update(b"s");
                self.put_utf8(h, *n, memo);
            }
            Some(Const::Class(n)) => {
                h.update(b"c");
                self.put_utf8(h, *n, memo);
            }
            Some(Const::Num(t, b)) => {
                h.update([b'n', *t]);
                h.update(b);
            }
            Some(Const::MethodType(d)) => {
                h.update(b"t");
                self.put_utf8(h, *d, memo);
            }
            Some(Const::Handle(kind, r)) => {
                h.update([b'h', *kind]);
                if !self.put_member(h, *r, (5..=9).contains(kind), calls, memo) {
                    h.update(r.to_be_bytes());
                }
            }
            Some(Const::Indy(bsm, nat) | Const::Dynamic(bsm, nat)) if depth < 4 => {
                // Each bootstrap method is resolved once per depth, so the work is at most 4
                // times its argument lists, however many constants share one or an argument
                // list repeats a constant.
                let d = match memo.done.get(&(*bsm, depth)) {
                    Some(d) => *d,
                    None => {
                        let mut s = Sha256::new();
                        match self.bootstrap.get(usize::from(*bsm)) {
                            Some((handle, args)) => {
                                self.put_const(&mut s, b'b', *handle, calls, memo, depth + 1);
                                s.update((args.len() as u32).to_be_bytes());
                                for &a in args {
                                    self.put_const(&mut s, b'a', a, calls, memo, depth + 1);
                                }
                            }
                            None => s.update(bsm.to_be_bytes()),
                        }
                        let d: [u8; 32] = s.finalize().into();
                        memo.done.insert((*bsm, depth), d);
                        d
                    }
                };
                h.update(b"d");
                h.update(d);
                match self.cp.get(usize::from(*nat)) {
                    Some(Const::NameType(n, d)) => {
                        self.put_utf8(h, *n, memo);
                        self.put_utf8(h, *d, memo);
                    }
                    _ => h.update(nat.to_be_bytes()),
                }
            }
            _ => h.update([b'?', (i >> 8) as u8, i as u8]),
        }
    }
}

/// A length-prefixed string, so two strings can't run together into the same bytes.
fn put(h: &mut Sha256, s: &str) {
    h.update((s.len() as u32).to_be_bytes());
    h.update(s.as_bytes());
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
    /// new on a class constant.
    New(u16),
    /// getstatic or putstatic on a field reference: the owner class gets initialized.
    Static(u16),
}

/// Walks bytecode and yields interesting instructions. Stops quietly at the first
/// malformed instruction; what was read so far still counts.
pub struct Ops<'a> {
    code: &'a [u8],
    pc: usize,
    /// The last two integer constants pushed, if nothing else has run since.
    pushes: [Option<i32>; 2],
    /// A constant byte or char array being filled, and the one finished before the last call.
    filling: Option<Vec<u16>>,
    finished: Option<Vec<u16>>,
    wide: bool,
}

pub fn ops(code: &[u8]) -> Ops<'_> {
    Ops {
        code,
        pc: 0,
        pushes: [None; 2],
        filling: None,
        finished: None,
        wide: false,
    }
}

impl Ops<'_> {
    /// Text of the constant array filled in before the call `next` just returned, as in
    /// `new String(new byte[] {104, 116, 116, 112})`. Three characters or more, no control
    /// characters.
    pub fn array_text(&self) -> Option<String> {
        let array = self.finished.as_ref()?;
        let text = if self.wide {
            String::from_utf16(array).ok()?
        } else {
            String::from_utf8(array.iter().map(|&b| b as u8).collect()).ok()?
        };
        (text.chars().count() >= 3 && !text.chars().any(char::is_control)).then_some(text)
    }

    /// A constant array compiles to newarray, then dup, index, value and a store per
    /// element. Anything that doesn't fit that stops the array.
    fn track_array(&mut self, pc: usize) {
        let op = self.code[pc];
        let pushed = match op {
            0x02..=0x08 => Some(i32::from(op) - 3),
            0x10 => Some(i32::from(self.code[pc + 1] as i8)),
            0x11 => self.u2(pc + 1).map(|v| i32::from(v as i16)),
            _ => None,
        };
        if pushed.is_some() {
            self.pushes = [self.pushes[1], pushed];
            return;
        }
        match op {
            0xbc => {
                let atype = self.code[pc + 1];
                self.wide = atype == 5;
                self.filling = matches!(atype, 5 | 8).then(Vec::new);
            }
            0x54 | 0x55 => match (self.filling.as_mut(), self.pushes) {
                (Some(array), [Some(index), Some(value)]) if index as usize == array.len() => {
                    array.push(value as u16)
                }
                _ => self.filling = None,
            },
            0xb6..=0xb9 => self.finished = self.filling.take(),
            _ => {}
        }
        self.pushes = [None; 2];
    }

    fn u2(&self, at: usize) -> Option<u16> {
        Some(u16::from_be_bytes([
            *self.code.get(at)?,
            *self.code.get(at + 1)?,
        ]))
    }
    fn i4(&self, at: usize) -> Option<i32> {
        Some(i32::from_be_bytes(
            self.code.get(at..at + 4)?.try_into().ok()?,
        ))
    }
    /// Length of the instruction at `pc`, or None if it's invalid.
    fn len(&self, pc: usize) -> Option<usize> {
        let op = *self.code.get(pc)?;
        Some(match op {
            0x10 | 0x12 | 0x15..=0x19 | 0x36..=0x3a | 0xa9 | 0xbc => 2,
            0x11
            | 0x13
            | 0x14
            | 0x84
            | 0x99..=0xa8
            | 0xb2..=0xb8
            | 0xbb
            | 0xbd
            | 0xc0
            | 0xc1
            | 0xc6
            | 0xc7 => 3,
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
            self.track_array(pc);
            let op = match self.code[pc] {
                0xb6..=0xb9 => Op::Invoke(self.u2(pc + 1)?),
                0xba => Op::Indy(self.u2(pc + 1)?),
                0x12 => Op::Ldc(u16::from(self.code[pc + 1])),
                0x13 | 0x14 => Op::Ldc(self.u2(pc + 1)?),
                0xbb => Op::New(self.u2(pc + 1)?),
                0xb2 | 0xb3 => Op::Static(self.u2(pc + 1)?),
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
        let mut b = vec![
            0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 5, 1, 0, 3, b'a', b'b', b'c',
        ];
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
    fn walker_reads_constant_arrays() {
        // Index, value and store for "abc", then the call that takes the array.
        let fill = |atype: u8, store: u8| {
            let mut code = vec![0x06, 0xbc, atype];
            for (i, c) in [97u8, 98, 99].iter().enumerate() {
                code.extend_from_slice(&[0x59, 0x03 + i as u8, 0x10, *c, store]);
            }
            code.extend_from_slice(&[0xb7, 0, 1]);
            code
        };
        let text = |code: &[u8]| {
            let mut walker = ops(code);
            walker.next();
            walker.array_text()
        };
        assert_eq!(text(&fill(8, 0x54)).as_deref(), Some("abc"));
        assert_eq!(text(&fill(5, 0x55)).as_deref(), Some("abc"));
        // An int array isn't text, and neither is an element the code computes.
        assert_eq!(text(&fill(10, 0x4f)), None);
        let mut computed = fill(8, 0x54);
        computed[4] = 0x60;
        assert_eq!(text(&computed), None);
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
