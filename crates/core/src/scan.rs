//! Walks the archive tree, collects rule hits, and builds the report.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use sha2::{Digest, Sha256};

use crate::archive::{self, ReadError};
use crate::class::{self, ClassFile, Const, Op};
use crate::meta;
use crate::report::*;
use crate::rules::{self, REFLECT_CLASS_ONLY};
use crate::strings::{self, Payload, UrlKind};
use crate::Limits;

const EVIDENCE_CAP: usize = 50;
pub(crate) const OBF_THRESHOLD: u32 = 25;

/// Text resources scanned for addresses and encoded payloads.
const TEXT_EXTS: &[&str] = &["yml", "yaml", "json", "properties", "txt", "toml", "conf", "cfg"];

pub fn run(name: &str, bytes: &[u8], limits: &Limits) -> (Report, Classes) {
    let mut s = Scan::new(limits);
    s.archive(bytes, "", 0);
    s.finish(name, bytes)
}

/// Per-class facts that the report doesn't carry, for `diff`.
pub struct Classes {
    /// Every class location: `a/B`, `lib.jar!/a/B`, `a/B!embedded-1!/c/D`.
    pub names: BTreeSet<String>,
    /// Notable and High rules with evidence in each class, before the evidence cap.
    pub rules: BTreeMap<String, BTreeSet<&'static str>>,
}

fn sha256(b: &[u8]) -> String {
    Sha256::digest(b).iter().map(|x| format!("{x:02x}")).collect()
}

fn ev(location: &str, method: Option<&str>, detail: impl Into<String>) -> Evidence {
    Evidence { location: location.to_string(), method: method.map(str::to_string), detail: detail.into() }
}

/// A call on a class inside the jar, resolved against its superclass chain at the end.
struct Pending {
    owner: String,
    name: String,
    desc: String,
    evidence: Evidence,
}

#[derive(Default)]
struct Obf {
    classes: u64,
    default_package: u64,
    names: u64,
    short_names: u64,
    long_strings: u64,
    encoded_strings: u64,
}

/// Per-class facts for the class-scoped correlations.
#[derive(Default)]
struct ClassFacts {
    rules: BTreeSet<&'static str>,
    chat_handler: bool,
    compares_literal: bool,
    jar_write: bool,
    plugins_ref: bool,
}

/// Per-method facts.
#[derive(Default)]
struct MethodFacts<'c> {
    /// String literals pushed since the last call; a call's string argument is the last one.
    since_call: Vec<&'c str>,
    reflect_classes: Vec<String>,
    reflect_names: Vec<String>,
    class_consts: Vec<&'c str>,
    reflect_lookup: bool,
    file_write: bool,
    jar_literal: bool,
    data_folder: bool,
    parent_file: bool,
    /// `URL.open*` calls, reported at the end of the method unless `local_url`.
    url_opens: Vec<Evidence>,
    /// The method got a URL from `getResource` or a local file.
    local_url: bool,
}

struct Scan<'l> {
    limits: &'l Limits,
    hits: BTreeMap<(&'static str, Option<String>), BTreeSet<Evidence>>,
    warnings: Vec<Warning>,
    partial: bool,
    total_bytes: u64,
    entries: u64,
    classes: u64,
    decoded: u32,
    decoded_bytes: u64,
    embedded_seq: u32,
    supers: HashMap<String, String>,
    pending: Vec<Pending>,
    top_classes: HashSet<String>,
    class_locs: BTreeSet<String>,
    obf: Obf,
    descriptors: Vec<(PluginInfo, String)>,
    libraries: BTreeSet<Library>,
    nested: Vec<Nested>,
}

impl<'l> Scan<'l> {
    fn new(limits: &'l Limits) -> Self {
        Scan {
            limits,
            hits: BTreeMap::new(),
            warnings: Vec::new(),
            partial: false,
            total_bytes: 0,
            entries: 0,
            classes: 0,
            decoded: 0,
            decoded_bytes: 0,
            embedded_seq: 0,
            supers: HashMap::new(),
            pending: Vec::new(),
            top_classes: HashSet::new(),
            class_locs: BTreeSet::new(),
            obf: Obf::default(),
            descriptors: Vec::new(),
            libraries: BTreeSet::new(),
            nested: Vec::new(),
        }
    }

    fn hit(&mut self, rule: &'static str, value: Option<String>, e: Evidence) {
        self.hits.entry((rule, value)).or_default().insert(e);
    }

    fn class_hit(&mut self, facts: &mut ClassFacts, rule: &'static str, e: Evidence) {
        facts.rules.insert(rule);
        self.hit(rule, None, e);
    }

    fn warn(&mut self, code: &str, location: &str, message: impl Into<String>) {
        self.warnings.push(Warning { code: code.into(), location: location.into(), message: message.into() });
    }

    fn limit(&mut self, code: &str, location: &str, message: impl Into<String>) {
        self.partial = true;
        self.warn(code, location, message);
    }

    fn archive(&mut self, data: &[u8], prefix: &str, depth: u32) {
        let remaining = self.limits.max_entries.saturating_sub(self.entries);
        let (entries, truncated) = match archive::entries(data, remaining) {
            Ok(x) => x,
            Err(e) => return self.warn("zip.unreadable", prefix.trim_end_matches("!/"), e),
        };
        if truncated {
            self.limit("zip.limit", prefix.trim_end_matches("!/"), format!("more than {} entries", self.limits.max_entries));
        }
        let mut seen = HashSet::new();
        for e in &entries {
            if e.name.ends_with('/') || e.name.ends_with('\\') {
                continue;
            }
            if self.entries >= self.limits.max_entries {
                return self.limit("zip.limit", prefix.trim_end_matches("!/"), format!("more than {} entries", self.limits.max_entries));
            }
            self.entries += 1;
            let (clean, unsafe_path) = archive::clean_name(&e.name);
            let path = format!("{prefix}{clean}");
            if unsafe_path {
                self.hit("jar.unsafe-path", Some(format!("{prefix}{}", e.name)), ev(&path, None, e.name.clone()));
            }
            if !seen.insert(clean.clone()) {
                self.warn("zip.duplicate-entry", &path, "another entry has the same name; both are scanned");
            }
            let left = self.limits.max_total_bytes.saturating_sub(self.total_bytes);
            if left == 0 {
                self.limit("zip.limit", &path, format!("total size above {} bytes; rest not scanned", self.limits.max_total_bytes));
                return;
            }
            let bytes = match archive::read(data, e, self.limits.max_entry_bytes.min(left), self.limits.max_ratio) {
                Ok(b) => b,
                Err(ReadError::Limit(m)) => {
                    self.limit("zip.limit", &path, m);
                    continue;
                }
                Err(ReadError::Unsupported(m)) => {
                    self.limit("zip.unsupported", &path, m);
                    continue;
                }
                Err(ReadError::Corrupt(m)) => {
                    self.limit("zip.bad-entry", &path, m);
                    continue;
                }
            };
            self.total_bytes += bytes.len() as u64;
            self.entry(&bytes, &path, &clean, prefix, depth);
        }
    }

    fn entry(&mut self, bytes: &[u8], path: &str, clean: &str, prefix: &str, depth: u32) {
        let file = clean.rsplit('/').next().unwrap_or(clean);
        let ext = file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
        let mismatch = |s: &mut Self, what: &str| {
            s.hit("res.type-mismatch", Some(path.to_string()), ev(path, None, what.to_string()));
        };

        if class::is_class(bytes) {
            if ext != "class" {
                mismatch(self, "contains a class file");
            }
            return self.class(bytes, prefix, path, depth);
        }
        if ext == "class" {
            return mismatch(self, "named .class but isn't a class file");
        }
        if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
            if !matches!(ext.as_str(), "jar" | "zip" | "war" | "jarinjar" | "mcpack") {
                mismatch(self, "contains a zip archive");
            }
            return self.nested(bytes, path, depth);
        }
        if let Some(kind) = native_kind(bytes) {
            if !matches!(ext.as_str(), "so" | "dll" | "dylib" | "jnilib" | "exe" | "bin" | "node") {
                mismatch(self, &format!("contains a {kind}"));
            }
            return;
        }

        if prefix.is_empty()
            && meta::platform_for(clean).is_some()
            && bytes.len() <= self.limits.max_text_bytes
        {
            let text = String::from_utf8_lossy(bytes);
            match meta::descriptor(clean, &text) {
                Some(info) => self.descriptors.push((info, path.to_string())),
                None => self.warn("descriptor.malformed", path, "couldn't be parsed"),
            }
        }
        if clean.starts_with("META-INF/maven/") && clean.ends_with("/pom.properties") {
            if let Some((group, artifact, version)) = meta::pom_properties(&String::from_utf8_lossy(bytes)) {
                self.libraries.insert(Library { group, artifact, version, location: path.to_string() });
            }
            return;
        }
        if clean.eq_ignore_ascii_case("META-INF/MANIFEST.MF") {
            for attr in meta::agent_attributes(&String::from_utf8_lossy(bytes)) {
                self.hit("code.instrument", None, ev(path, None, format!("manifest declares {attr}")));
            }
            return;
        }
        let text_ext = TEXT_EXTS.contains(&ext.as_str()) || meta::platform_for(clean).is_some();
        if bytes.len() <= self.limits.max_text_bytes {
            if text_ext {
                self.text(&String::from_utf8_lossy(bytes), path, depth);
            } else if !file.contains('.')
                && let Ok(t) = std::str::from_utf8(bytes)
            {
                self.text(t, path, depth);
            }
        }
    }

    fn nested(&mut self, bytes: &[u8], path: &str, depth: u32) {
        self.hit("jar.nested", Some(path.to_string()), ev(path, None, "nested archive"));
        self.nested.push(Nested { path: path.to_string(), size: bytes.len() as u64, sha256: sha256(bytes) });
        if depth >= self.limits.max_depth {
            return self.limit("nested.too-deep", path, format!("deeper than {} levels; not scanned", self.limits.max_depth));
        }
        self.archive(bytes, &format!("{path}!/"), depth + 1);
    }

    fn text(&mut self, text: &str, path: &str, depth: u32) {
        let is_sep = |c: char| c.is_whitespace() || "\"'<>(){}[],;=`".contains(c);
        for token in text.split(is_sep).filter(|t| !t.is_empty()) {
            self.literal(token, path, None, depth);
        }
        // A payload wrapped across lines, like MIME base64.
        let joined: String = text.split_whitespace().collect();
        if joined.len() >= 64 && joined.len() != text.trim().len() {
            self.payload(&joined, path, None, depth);
        }
    }

    /// Address, IP and payload checks for one string literal or text token.
    fn literal(&mut self, s: &str, location: &str, method: Option<&str>, depth: u32) {
        for url in strings::urls(s) {
            let e = ev(location, method, url.text);
            match url.kind {
                UrlKind::Host(h) => self.hit("str.url", Some(h), e),
                UrlKind::Ip(ip) => self.hit("str.ip", Some(ip), e),
                UrlKind::Discord { id } => self.hit("str.discord-webhook", Some(id), e),
                UrlKind::Telegram { id } => self.hit("str.telegram-bot", Some(id), e),
            }
        }
        if let Some(h) = strings::bare_host(s) {
            self.hit("str.url", Some(h.to_string()), ev(location, method, strings::cut(s.trim(), 200)));
        }
        if s.contains('.') && !s.contains("://") {
            for ip in strings::ipv4s(s) {
                self.hit("str.ip", Some(ip), ev(location, method, strings::cut(s, 200)));
            }
        }
        self.payload(s, location, method, depth);
    }

    fn payload(&mut self, s: &str, location: &str, method: Option<&str>, depth: u32) {
        let Some((kind, enc)) = strings::payload(s) else { return };
        let estimate = s.len() as u64;
        if self.decoded >= self.limits.max_decoded || self.decoded_bytes + estimate > self.limits.max_decoded_bytes {
            if !self.warnings.iter().any(|w| w.code == "decode.limit") {
                self.limit("decode.limit", location, "decoded-payload budget used up; later payloads not scanned");
            }
            return;
        }
        let Some(bytes) = strings::decode(s, &enc) else { return };
        self.decoded += 1;
        self.decoded_bytes += bytes.len() as u64;
        self.embedded_seq += 1;
        let label = format!("{location}!embedded-{}", self.embedded_seq);
        let (what, encoding) = (
            match kind {
                Payload::Class => "class file",
                Payload::Zip => "zip archive",
            },
            match enc {
                strings::Encoding::Base64 => "base64",
                strings::Encoding::Hex => "hex",
            },
        );
        self.hit("blob.embedded-code", Some(sha256(&bytes)), ev(location, method, format!("{encoding} string decodes to a {what}; scanned as {label}")));
        if depth >= self.limits.max_depth {
            return self.limit("nested.too-deep", &label, format!("deeper than {} levels; not scanned", self.limits.max_depth));
        }
        self.total_bytes += bytes.len() as u64;
        let prefix = format!("{label}!/");
        match kind {
            Payload::Class => self.class(&bytes, &prefix, &label, depth + 1),
            Payload::Zip => self.archive(&bytes, &prefix, depth + 1),
        }
    }

    fn class(&mut self, bytes: &[u8], prefix: &str, path: &str, depth: u32) {
        let cf = match ClassFile::parse(bytes) {
            Ok(cf) => cf,
            Err(e) => return self.warn("class.malformed", path, e),
        };
        let Some(name) = cf.name() else {
            return self.warn("class.malformed", path, "missing class name");
        };
        self.classes += 1;
        let loc = format!("{prefix}{name}");
        self.class_locs.insert(loc.clone());
        if let Some(sup) = cf.super_name() {
            self.supers.entry(name.to_string()).or_insert_with(|| sup.to_string());
        }
        let top = prefix.is_empty();
        if top {
            self.top_classes.insert(name.to_string());
            self.obf_class(&cf, name);
        }

        let mut facts = ClassFacts::default();
        if cf.interfaces.iter().any(|&i| cf.class_name(i) == Some("java/lang/instrument/ClassFileTransformer")) {
            self.class_hit(&mut facts, "code.instrument", ev(&loc, None, "implements java/lang/instrument/ClassFileTransformer"));
        }

        let mut seen_strings = HashSet::new();
        for m in &cf.methods {
            let (Some(mname), Some(mdesc)) = (cf.utf8(m.name), cf.utf8(m.desc)) else { continue };
            if rules::CHAT_HANDLER_DESCS.contains(&mdesc) {
                facts.chat_handler = true;
            }
            let Some(code) = m.code else { continue };
            let method = format!("{mname}{mdesc}");
            let mut mf = MethodFacts::default();
            for op in class::ops(code) {
                match op {
                    Op::Invoke(i) => {
                        if let Some((o, n, d)) = cf.member(i) {
                            self.call(&mut facts, &mut mf, &loc, &method, o, n, d);
                        }
                        mf.since_call.clear();
                    }
                    Op::Indy(i) => {
                        if let Some(Const::Indy(bsm)) = cf.cp.get(usize::from(i))
                            && let Some((handle, args)) = cf.bootstrap.get(usize::from(*bsm))
                        {
                            if let Some((o, n, d)) = cf.handle(*handle) {
                                self.call(&mut facts, &mut mf, &loc, &method, o, n, d);
                            }
                            for &arg in args {
                                if let Some((o, n, d)) = cf.handle(arg) {
                                    self.call(&mut facts, &mut mf, &loc, &method, o, n, d);
                                } else if let Some(s) = cf.string(arg) {
                                    seen_strings.insert(arg);
                                    self.literal(s, &loc, Some(&method), depth);
                                }
                            }
                        }
                        mf.since_call.clear();
                    }
                    Op::Ldc(i) => match cf.cp.get(usize::from(i)) {
                        Some(Const::Str(_)) => {
                            let Some(s) = cf.string(i) else { continue };
                            seen_strings.insert(i);
                            mf.since_call.push(s);
                            let lower = s.to_ascii_lowercase();
                            if lower.ends_with(".jar") || lower.ends_with(".class") {
                                mf.jar_literal = true;
                            }
                            if is_plugins_dir(s) {
                                facts.plugins_ref = true;
                            }
                            self.literal(s, &loc, Some(&method), depth);
                        }
                        Some(Const::Class(_)) => mf.class_consts.extend(cf.class_name(i)),
                        Some(Const::Handle(..)) => {
                            if let Some((o, n, d)) = cf.handle(i) {
                                self.call(&mut facts, &mut mf, &loc, &method, o, n, d);
                            }
                        }
                        _ => {}
                    },
                }
            }
            self.end_method(&mut facts, mf, &loc, &method);
        }

        // String constants only reachable from fields or annotations, not from code.
        for i in 0..cf.cp.len() as u16 {
            if !seen_strings.contains(&i)
                && let Some(s) = cf.string(i)
            {
                self.literal(s, &loc, None, depth);
            }
        }

        if facts.chat_handler && facts.compares_literal {
            for part in ["mc.set-op", "mc.console-command"] {
                if facts.rules.contains(part) {
                    self.hit("corr.hidden-command", None, ev(&loc, None, format!("chat or command handler, string-literal comparison and {part} in one class")));
                }
            }
        }
        if facts.jar_write && facts.plugins_ref {
            self.hit("corr.modify-plugins", None, ev(&loc, None, "writes jar or class files and refers to the plugins folder in one class"));
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn call<'c>(&mut self, facts: &mut ClassFacts, mf: &mut MethodFacts<'c>, loc: &str, method: &str, owner: &'c str, name: &'c str, desc: &'c str) {
        let detail = format!("{owner}.{name}");
        for rule in rules::match_call(owner, name, Some(desc)) {
            // A class's own serialization hook reads its fields back; that isn't reading
            // objects from somewhere else.
            if rule == "deser.java" && method == "readObject(Ljava/io/ObjectInputStream;)V" {
                continue;
            }
            if owner == "java/net/URL" {
                mf.url_opens.push(ev(loc, Some(method), detail.clone()));
                continue;
            }
            if rule == "fs.write-archive" && owner == "java/util/jar/JarOutputStream" {
                facts.jar_write = true;
            }
            self.class_hit(facts, rule, ev(loc, Some(method), detail.clone()));
        }
        if may_be_in_jar(owner) && rules::is_inheritable_name(name) {
            self.pending.push(Pending {
                owner: owner.to_string(),
                name: name.to_string(),
                desc: desc.to_string(),
                evidence: ev(loc, Some(method), detail),
            });
        }

        let last = mf.since_call.last().map(|s| s.to_string());
        match (owner, name) {
            ("java/lang/String", n) if rules::STRING_COMPARES.contains(&n) && last.is_some() => facts.compares_literal = true,
            ("java/lang/Class", "forName") | ("java/lang/ClassLoader", "loadClass") | ("java/lang/invoke/MethodHandles$Lookup", "findClass") => {
                mf.reflect_classes.extend(last.map(|s| s.replace('.', "/")));
            }
            ("java/lang/Class", "getMethod" | "getDeclaredMethod")
            | ("java/lang/invoke/MethodHandles$Lookup", "findStatic" | "findVirtual" | "findSpecial") => {
                mf.reflect_names.extend(last);
                mf.reflect_lookup = true;
            }
            ("java/lang/Class", "getConstructor" | "getDeclaredConstructor") | ("java/lang/invoke/MethodHandles$Lookup", "findConstructor") => {
                mf.reflect_names.push("<init>".into());
                mf.reflect_lookup = true;
            }
            ("java/io/File", "getParentFile") => mf.parent_file = true,
            // ponytail: method-level, so a method that opens a resource and a remote URL
            // loses the remote one; track URL values through locals if that shows up.
            (_, "getResource" | "getSystemResource") if desc.ends_with(")Ljava/net/URL;") => mf.local_url = true,
            ("java/io/File", "toURI" | "toURL") | ("java/nio/file/Path", "toUri") => mf.local_url = true,
            (o, "getPluginsFolder" | "getUpdateFolder" | "getUpdateFolderFile") if o.starts_with("org/bukkit/") => facts.plugins_ref = true,
            (_, "getDataFolder") if desc == "()Ljava/io/File;" => mf.data_folder = true,
            _ => {}
        }
        if rules::is_file_write(owner, name) {
            mf.file_write = true;
        }
    }

    fn end_method(&mut self, facts: &mut ClassFacts, mf: MethodFacts, loc: &str, method: &str) {
        if !mf.local_url {
            for e in mf.url_opens.iter().cloned() {
                self.class_hit(facts, "net.http", e);
            }
        }
        if mf.jar_literal && mf.file_write {
            facts.jar_write = true;
            self.class_hit(facts, "fs.write-archive", ev(loc, Some(method), "writes a file named .jar or .class"));
        }
        if mf.data_folder && mf.parent_file {
            facts.plugins_ref = true;
        }
        let mut owners: Vec<String> = mf.reflect_classes.clone();
        if mf.reflect_lookup {
            owners.extend(mf.class_consts.iter().map(|s| s.to_string()));
        }
        for owner in &owners {
            for name in &mf.reflect_names {
                for rule in rules::match_call(owner, name, None).collect::<Vec<_>>() {
                    self.class_hit(facts, rule, ev(loc, Some(method), format!("via reflection: {owner}.{name}")));
                    self.class_hit(facts, "reflect.indirect", ev(loc, Some(method), format!("{owner}.{name}")));
                }
            }
        }
        for owner in &mf.reflect_classes {
            if let Some((_, rule)) = REFLECT_CLASS_ONLY.iter().find(|(c, _)| c == owner) {
                self.class_hit(facts, rule, ev(loc, Some(method), format!("via reflection: {owner}")));
                self.class_hit(facts, "reflect.indirect", ev(loc, Some(method), owner.clone()));
            }
        }
    }

    fn obf_class(&mut self, cf: &ClassFile, name: &str) {
        self.obf.classes += 1;
        if !name.contains('/') {
            self.obf.default_package += 1;
        }
        let simple = name.rsplit('/').next().unwrap_or(name);
        let simple = simple.rsplit('$').next().unwrap_or(simple);
        let mut names: Vec<&str> = Vec::new();
        if !simple.bytes().all(|c| c.is_ascii_digit()) {
            names.push(simple);
        }
        names.extend(cf.methods.iter().filter_map(|m| cf.utf8(m.name)).filter(|n| {
            !matches!(*n, "<init>" | "<clinit>" | "values" | "valueOf") && !n.starts_with("lambda$") && !n.starts_with("access$")
        }));
        names.extend(cf.fields.iter().filter_map(|&f| cf.utf8(f)));
        for n in names {
            self.obf.names += 1;
            if short_or_garbled(n) {
                self.obf.short_names += 1;
            }
        }
        for c in &cf.cp {
            if let Const::Str(i) = c
                && let Some(s) = cf.utf8(*i)
                && s.chars().count() >= 16
            {
                self.obf.long_strings += 1;
                if strings::looks_encoded(s) {
                    self.obf.encoded_strings += 1;
                }
            }
        }
    }

    fn finish(mut self, name: &str, bytes: &[u8]) -> (Report, Classes) {
        // Calls on in-jar subclasses of library classes (a custom ClassLoader, say).
        for p in std::mem::take(&mut self.pending) {
            let mut owner = p.owner.as_str();
            let mut steps = 0;
            while let Some(sup) = self.supers.get(owner) {
                owner = sup;
                steps += 1;
                if steps > 64 {
                    break;
                }
            }
            if owner != p.owner {
                let owner = owner.to_string();
                let rules: Vec<_> = rules::match_call(&owner, &p.name, Some(&p.desc)).collect();
                for rule in rules {
                    let mut e = p.evidence.clone();
                    e.detail = format!("{} (subclass of {owner})", e.detail);
                    self.hit(rule, None, e);
                }
            }
        }

        // Correlations. Each part is a list of alternatives; the first evidence of each
        // part goes into the finding. Class scope groups evidence by its location.
        const NET: &[&str] = &["net.http", "net.socket", "str.url"];
        let correlations: [(&'static str, bool, [&[&str]; 2]); 3] = [
            ("corr.download-and-run", true, [NET, &["code.load"]]),
            ("corr.command-and-network", true, [&["exec.process"], NET]),
            ("corr.hidden-loader", false, [&["blob.embedded-code"], &["code.load"]]),
        ];
        let mut found = Vec::new();
        for (corr, class_scope, parts) in correlations {
            let mut by_scope: BTreeMap<&str, [Option<Evidence>; 2]> = BTreeMap::new();
            for ((rule, _), evidence) in &self.hits {
                for (i, part) in parts.iter().enumerate() {
                    if !part.contains(rule) {
                        continue;
                    }
                    for e in evidence {
                        let scope = if class_scope { e.location.as_str() } else { "" };
                        let slot = &mut by_scope.entry(scope).or_default()[i];
                        if slot.is_none() {
                            *slot = Some(Evidence { detail: format!("{rule}: {}", e.detail), ..e.clone() });
                        }
                    }
                }
            }
            for [a, b] in by_scope.into_values() {
                if let (Some(a), Some(b)) = (a, b) {
                    found.push((corr, a));
                    found.push((corr, b));
                }
            }
        }
        for (corr, e) in found {
            self.hit(corr, None, e);
        }

        let mut plugins = Vec::new();
        for (info, path) in std::mem::take(&mut self.descriptors) {
            if matches!(info.platform.as_str(), "bukkit" | "paper" | "bungee" | "velocity") {
                for class in [&info.main, &info.bootstrapper, &info.loader].into_iter().flatten() {
                    if !self.top_classes.contains(&class.replace('.', "/")) {
                        self.hit("meta.missing-main", Some(class.clone()), ev(&path, None, format!("declared class {class} isn't in the jar")));
                    }
                }
            }
            plugins.push(info);
        }
        plugins.sort_by(|a, b| a.descriptor.cmp(&b.descriptor));
        let unreadable = self.warnings.iter().any(|w| w.code == "zip.unreadable" && w.location.is_empty());
        if plugins.is_empty() && !unreadable {
            self.hit("meta.no-descriptor", None, ev("", None, "no plugin or mod descriptor at the jar root"));
        }

        let libraries: Vec<Library> = std::mem::take(&mut self.libraries).into_iter().collect();
        for lib in &libraries {
            if meta::log4j_vulnerable(&lib.group, &lib.artifact, &lib.version) {
                self.hit("lib.log4j-vulnerable", None, ev(&lib.location, None, format!("{}:{}:{}", lib.group, lib.artifact, lib.version)));
            }
        }

        let ratio = |a: u64, b: u64| if b == 0 { 0.0 } else { a as f64 / b as f64 };
        let round = |x: f64| (x * 1000.0).round() / 1000.0;
        let o = &self.obf;
        let (sn, dp, es) = (ratio(o.short_names, o.names), ratio(o.default_package, o.classes), ratio(o.encoded_strings, o.long_strings));
        // Small samples are scaled down so one API key or a couple of short names can't
        // make a tiny jar look obfuscated.
        let damped = 0.5 * ratio(o.short_names, o.names.max(20))
            + 0.2 * ratio(o.default_package, o.classes.max(5))
            + 0.3 * ratio(o.encoded_strings, o.long_strings.max(20));
        let score = (100.0 * damped.min(1.0)).round() as u32;
        if score >= OBF_THRESHOLD {
            self.hit("obf.score", None, ev("", None, format!("short or garbled names {:.0}%, default package {:.0}%, encoded strings {:.0}%", sn * 100.0, dp * 100.0, es * 100.0)));
        }

        let mut class_rules: BTreeMap<String, BTreeSet<&'static str>> = BTreeMap::new();
        for ((id, _), evidence) in &self.hits {
            if rules::rule(id).severity == Severity::Info {
                continue;
            }
            for e in evidence.iter().filter(|e| self.class_locs.contains(&e.location)) {
                class_rules.entry(e.location.clone()).or_default().insert(id);
            }
        }
        let classes = Classes { names: std::mem::take(&mut self.class_locs), rules: class_rules };

        let mut findings: Vec<Finding> = self
            .hits
            .into_iter()
            .map(|((id, value), evidence)| {
                let r = rules::rule(id);
                let title = if id == "obf.score" { format!("{} (score {score}/100)", r.title) } else { r.title.to_string() };
                Finding {
                    rule: id.to_string(),
                    severity: r.severity,
                    title,
                    why: r.why.to_string(),
                    legit_uses: r.legit_uses.to_string(),
                    value,
                    evidence_total: evidence.len(),
                    evidence: evidence.into_iter().take(EVIDENCE_CAP).collect(),
                }
            })
            .collect();
        findings.sort_by(|a, b| b.severity.cmp(&a.severity).then_with(|| a.rule.cmp(&b.rule)).then_with(|| a.value.cmp(&b.value)));

        let report = Report {
            schema_version: SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            file: FileInfo { name: name.to_string(), size: bytes.len() as u64, sha256: sha256(bytes) },
            partial: self.partial,
            plugins,
            libraries,
            nested: self.nested,
            obfuscation: Obfuscation { score, short_names: round(sn), default_package: round(dp), encoded_strings: round(es) },
            findings,
            warnings: self.warnings,
            stats: Stats { entries: self.entries, classes: self.classes, uncompressed_bytes: self.total_bytes },
            disclaimer: DISCLAIMER.to_string(),
        };
        (report, classes)
    }
}

fn may_be_in_jar(owner: &str) -> bool {
    !["java/", "javax/", "jdk/", "sun/", "com/sun/", "kotlin/", "org/bukkit/", "net/md_5/", "com/velocitypowered/"]
        .iter()
        .any(|p| owner.starts_with(p))
}

fn is_plugins_dir(s: &str) -> bool {
    let s = s.strip_prefix("./").unwrap_or(s);
    s == "plugins" || s.starts_with("plugins/") || s.starts_with("plugins\\")
}

const JAVA_KEYWORDS: &[&str] = &[
    "abstract", "assert", "boolean", "break", "byte", "case", "catch", "char", "class", "const", "continue",
    "default", "do", "double", "else", "enum", "extends", "final", "finally", "float", "for", "goto", "if",
    "implements", "import", "instanceof", "int", "interface", "long", "native", "new", "package", "private",
    "protected", "public", "return", "short", "static", "strictfp", "super", "switch", "synchronized", "this",
    "throw", "throws", "transient", "try", "void", "volatile", "while",
];

fn short_or_garbled(n: &str) -> bool {
    n.chars().count() <= 2
        || !n.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'$')
        || JAVA_KEYWORDS.contains(&n)
}

fn native_kind(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(b"\x7fELF") {
        return Some("Linux program or library (ELF)");
    }
    if b.len() >= 64 && b.starts_with(b"MZ") {
        let off = u32::from_le_bytes([b[60], b[61], b[62], b[63]]) as usize;
        if b.get(off..off + 4) == Some(b"PE\0\0") {
            return Some("Windows program or library (PE)");
        }
    }
    let m = b.get(..4)?;
    if [[0xFE, 0xED, 0xFA, 0xCE], [0xFE, 0xED, 0xFA, 0xCF], [0xCE, 0xFA, 0xED, 0xFE], [0xCF, 0xFA, 0xED, 0xFE]].iter().any(|x| x == m)
        || (m == [0xCA, 0xFE, 0xBA, 0xBE] && !class::is_class(b))
    {
        return Some("macOS program or library (Mach-O)");
    }
    None
}
