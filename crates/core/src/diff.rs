//! Compares the scans of two versions of a jar. Findings are matched by rule id and
//! value, never by evidence, so code that only moved isn't reported as a change.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::report::*;
use crate::rules;
use crate::scan::{ClassPrint, Classes, OBF_THRESHOLD};

/// Below this share of old class names surviving, an obfuscated pair was probably
/// re-obfuscated and "new classes" means nothing.
const RENAMED_SHARE: f64 = 0.5;

type Versions = BTreeMap<String, BTreeSet<String>>;
type MakeItem = fn(String) -> Item;
type Field = fn(&PluginInfo) -> &[String];
type VersionsOf = fn(&Report) -> Versions;

#[derive(Debug, Clone, Serialize)]
pub struct Diff {
    pub schema_version: u32,
    pub tool_version: String,
    pub old: Side,
    pub new: Side,
    pub obfuscation: OldNew<u32>,
    /// What the new version adds, most severe first.
    pub added: Vec<Item>,
    pub removed: Vec<Item>,
    pub changed: Vec<Change>,
    /// What changed in the code, class by class, and in the other files.
    pub code: CodeDiff,
    /// Both jars run the same code: the same classes, method by method, the same native
    /// libraries, the same main class, bootstrapper, loader and descriptor `libraries:`,
    /// and both were read in full. Other files may still differ (see `code`).
    pub same_code: bool,
    /// Findings present in both versions.
    pub unchanged: Vec<Key>,
    pub warnings: Vec<Warning>,
    pub disclaimer: String,
}

impl Diff {
    /// The new version adds at least one High finding.
    pub fn adds_high(&self) -> bool {
        self.added
            .iter()
            .any(|i| matches!(i, Item::Finding { finding } if finding.severity == Severity::High))
    }

    /// Either file couldn't be opened as a zip, so the comparison means nothing.
    pub fn is_unreadable(&self) -> bool {
        unreadable(&self.old.warnings) || unreadable(&self.new.warnings)
    }

    /// See the `same_code` field. False when either side is unreadable or partial, or the
    /// classes were renamed, since then not everything was compared.
    pub fn same_code(&self) -> bool {
        self.same_code
    }
}

/// What changed in the code, class by class, and in the other files. Classes are
/// compared method by method on their instructions and the constants they use, so a
/// recompile that only renumbers the constant pool, or a different zip, isn't a change.
/// Empty when the jar was re-obfuscated (see the `diff.classes-renamed` warning).
#[derive(Debug, Clone, Default, Serialize)]
pub struct CodeDiff {
    /// Classes in both versions whose code differs, most severe first.
    pub changed_classes: Vec<ClassChange>,
    /// Every class file only in the new version, with or without findings.
    pub added_classes: Vec<String>,
    pub removed_classes: Vec<String>,
    /// Class files whose code is the same in both versions.
    pub unchanged_classes: u64,
    /// Other files, by content. A nested jar counts by the files inside it; jar
    /// signature files and Maven build metadata are left out.
    pub added_files: Vec<String>,
    pub removed_files: Vec<String>,
    pub changed_files: Vec<String>,
}

/// A class file in both versions with different code. Names are the class file's path
/// without `.class`; methods are `name(descriptor)`, fields `name:descriptor`.
#[derive(Debug, Clone, Serialize)]
pub struct ClassChange {
    pub class: String,
    /// That of the most severe rule in `rules_added`, else info.
    pub severity: Severity,
    /// Notable and High rules with evidence in this class that the old class didn't have.
    pub rules_added: Vec<String>,
    /// Superclass, interfaces, class access flags or class annotations differ.
    pub header_changed: bool,
    pub methods_added: Vec<String>,
    pub methods_removed: Vec<String>,
    /// Same name and descriptor, different access flags, annotations, code or exception
    /// handlers.
    pub methods_changed: Vec<String>,
    pub fields_added: Vec<String>,
    pub fields_removed: Vec<String>,
    /// Same name and type, different access flags, constant value or annotations.
    pub fields_changed: Vec<String>,
    /// Methods the class calls now that it didn't before, `owner.name`.
    pub calls_added: Vec<String>,
    /// A jar has several different entries with this class's path, and the server may
    /// load any of them.
    pub duplicate: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Side {
    pub file: FileInfo,
    pub plugins: Vec<PluginInfo>,
    pub partial: bool,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OldNew<T> {
    pub old: T,
    pub new: T,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Item {
    Finding {
        finding: Finding,
    },
    /// A class that exists only on this side and carries notable or High evidence.
    /// `severity` is that of its most severe rule.
    Class {
        value: String,
        rules: Vec<String>,
        severity: Severity,
    },
    Command {
        value: String,
    },
    Permission {
        value: String,
    },
    /// A `libraries:` entry from a descriptor, `group:artifact:version`.
    MavenLibrary {
        value: String,
    },
    /// A bundled library from `pom.properties`, `group:artifact:version`.
    Library {
        value: String,
    },
    /// A plugin this one needs to load.
    Dependency {
        value: String,
    },
    /// A plugin this one uses when it's installed.
    OptionalDependency {
        value: String,
    },
}

impl Item {
    pub fn severity(&self) -> Severity {
        match self {
            Item::Finding { finding } => finding.severity,
            Item::Class { severity, .. } => *severity,
            _ => Severity::Info,
        }
    }

    fn sort_key(&self) -> (Reverse<Severity>, u8, String) {
        let (kind, key) = match self {
            Item::Finding { finding } => (
                0,
                format!(
                    "{}\0{}",
                    finding.rule,
                    finding.value.as_deref().unwrap_or("")
                ),
            ),
            Item::Class { value, .. } => (1, value.clone()),
            Item::Command { value } => (2, value.clone()),
            Item::Permission { value } => (3, value.clone()),
            Item::MavenLibrary { value } => (4, value.clone()),
            Item::Library { value } => (5, value.clone()),
            Item::Dependency { value } => (6, value.clone()),
            Item::OptionalDependency { value } => (7, value.clone()),
        };
        (Reverse(self.severity()), kind, key)
    }
}

/// A value present in both versions that differs: a library version, a nested jar's
/// hash, or a descriptor field. `None` means the side doesn't have it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Change {
    /// `library`, `maven-library`, `nested` or `plugin-field`.
    pub kind: &'static str,
    pub key: String,
    pub old: Option<String>,
    pub new: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Key {
    pub rule: String,
    pub value: Option<String>,
}

pub fn diff((old, old_classes): (Report, Classes), (new, new_classes): (Report, Classes)) -> Diff {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    let mut warnings = Vec::new();

    let keyed = |r: &Report| -> BTreeMap<Key, Finding> {
        r.findings
            .iter()
            .map(|f| {
                (
                    Key {
                        rule: f.rule.clone(),
                        value: f.value.clone(),
                    },
                    f.clone(),
                )
            })
            .collect()
    };
    let (old_f, mut new_f) = (keyed(&old), keyed(&new));
    let mut unchanged = Vec::new();
    for (key, finding) in old_f {
        match new_f.remove(&key) {
            Some(_) => unchanged.push(key),
            None => removed.push(Item::Finding { finding }),
        }
    }
    added.extend(new_f.into_values().map(|finding| Item::Finding { finding }));

    let kept = old_classes.names.intersection(&new_classes.names).count();
    let obfuscated =
        old.obfuscation.score >= OBF_THRESHOLD || new.obfuscation.score >= OBF_THRESHOLD;
    if obfuscated
        && !old_classes.names.is_empty()
        && (kept as f64) < RENAMED_SHARE * old_classes.names.len() as f64
    {
        warnings.push(Warning {
            code: "diff.classes-renamed".into(),
            location: String::new(),
            message: format!(
                "only {kept} of {} class names are still there and the code is obfuscated, so the jar was probably re-obfuscated; new and removed classes aren't listed",
                old_classes.names.len()
            ),
        });
    } else {
        let only = |a: &Classes, b: &Classes| -> Vec<Item> {
            a.rules
                .iter()
                .filter(|(class, _)| !b.names.contains(*class))
                .map(|(class, ids)| Item::Class {
                    value: class.clone(),
                    rules: ids.iter().map(|r| r.to_string()).collect(),
                    severity: ids
                        .iter()
                        .map(|r| rules::rule(r).severity)
                        .max()
                        .unwrap_or(Severity::Info),
                })
                .collect()
        };
        added.extend(only(&new_classes, &old_classes));
        removed.extend(only(&old_classes, &new_classes));
    }
    let renamed = warnings.iter().any(|w| w.code == "diff.classes-renamed");
    let code = if renamed {
        CodeDiff::default()
    } else {
        code_diff(&old_classes, &new_classes)
    };

    let values = |r: &Report, f: fn(&PluginInfo) -> &[String]| -> BTreeSet<String> {
        r.plugins
            .iter()
            .flat_map(|p| f(p).iter().cloned())
            .collect()
    };
    let lists: [(Field, MakeItem); 4] = [
        (|p| &p.commands, |value| Item::Command { value }),
        (|p| &p.permissions, |value| Item::Permission { value }),
        (|p| &p.dependencies, |value| Item::Dependency { value }),
        (
            |p| &p.optional_dependencies,
            |value| Item::OptionalDependency { value },
        ),
    ];
    for (field, make) in lists {
        let (a, b) = (values(&old, field), values(&new, field));
        added.extend(b.difference(&a).cloned().map(make));
        removed.extend(a.difference(&b).cloned().map(make));
    }

    // Libraries are keyed by group:artifact so a version bump is a change, not add + remove.
    let libs: [(&str, VersionsOf, MakeItem); 2] = [
        ("maven-library", maven_versions, |value| {
            Item::MavenLibrary { value }
        }),
        ("library", bundled_versions, |value| Item::Library { value }),
    ];
    for (kind, versions, make) in libs {
        let (a, b) = (versions(&old), versions(&new));
        let coord = |k: &str, v: &BTreeSet<String>| {
            v.iter()
                .map(|v| {
                    if v.is_empty() {
                        k.to_string()
                    } else {
                        format!("{k}:{v}")
                    }
                })
                .collect::<Vec<_>>()
        };
        for (k, v) in &b {
            match a.get(k) {
                None => added.extend(coord(k, v).into_iter().map(make)),
                Some(old_v) if old_v != v => changed.push(Change {
                    kind,
                    key: k.clone(),
                    old: Some(join(old_v)),
                    new: Some(join(v)),
                }),
                Some(_) => {}
            }
        }
        for (k, v) in &a {
            if !b.contains_key(k) {
                removed.extend(coord(k, v).into_iter().map(make));
            }
        }
    }

    // Added and removed nested jars are `jar.nested` findings; only content changes go here.
    let nested: BTreeMap<&str, &str> = old
        .nested
        .iter()
        .map(|n| (n.path.as_str(), n.sha256.as_str()))
        .collect();
    for n in &new.nested {
        if let Some(&sha) = nested.get(n.path.as_str())
            && sha != n.sha256
        {
            changed.push(Change {
                kind: "nested",
                key: n.path.clone(),
                old: Some(sha.to_string()),
                new: Some(n.sha256.clone()),
            });
        }
    }

    let descriptors: BTreeSet<&str> = old
        .plugins
        .iter()
        .chain(&new.plugins)
        .map(|p| p.descriptor.as_str())
        .collect();
    for d in descriptors {
        let find = |r: &'_ Report| {
            r.plugins
                .iter()
                .find(|p| p.descriptor == d)
                .cloned()
                .unwrap_or_default()
        };
        let (a, b) = (find(&old), find(&new));
        // Authors in any order, so reordering them isn't a change.
        let authors = |p: &PluginInfo| {
            Some(join(&p.authors.iter().cloned().collect())).filter(|s| !s.is_empty())
        };
        let (old_authors, new_authors) = (authors(&a), authors(&b));
        let fields = [
            ("name", a.name, b.name),
            ("version", a.version, b.version),
            ("main", a.main, b.main),
            ("bootstrapper", a.bootstrapper, b.bootstrapper),
            ("loader", a.loader, b.loader),
            ("authors", old_authors, new_authors),
            ("website", a.website, b.website),
        ];
        for (field, x, y) in fields {
            if x != y {
                changed.push(Change {
                    kind: "plugin-field",
                    key: format!("{d} {field}"),
                    old: x,
                    new: y,
                });
            }
        }
    }

    // Code also runs from native libraries, and from what the descriptor tells the server
    // to start or download.
    let native = |p: &String| old_classes.natives.contains(p) || new_classes.natives.contains(p);
    let natives: Vec<&String> = code
        .added_files
        .iter()
        .chain(&code.removed_files)
        .chain(&code.changed_files)
        .filter(|p| native(p))
        .collect();
    let natives_differ = !natives.is_empty();
    if let [first, rest @ ..] = &natives[..] {
        warnings.push(Warning {
            code: "diff.native-changed".into(),
            location: (*first).clone(),
            message: match rest.len() {
                0 => "a native library differs, and jarsnitch doesn't read native code".into(),
                n => format!(
                    "{} native libraries differ, and jarsnitch doesn't read native code",
                    n + 1
                ),
            },
        });
    }
    let start_differs = changed.iter().any(|c| {
        c.kind == "maven-library"
            || (c.kind == "plugin-field"
                && [" main", " bootstrapper", " loader"]
                    .iter()
                    .any(|f| c.key.ends_with(f)))
    }) || added
        .iter()
        .chain(&removed)
        .any(|i| matches!(i, Item::MavenLibrary { .. }));
    let code_differs = !(code.changed_classes.is_empty()
        && code.added_classes.is_empty()
        && code.removed_classes.is_empty())
        || natives_differ
        || start_differs;
    let same_code = !code_differs
        && !renamed
        && !old.partial
        && !new.partial
        && !unreadable(&old.warnings)
        && !unreadable(&new.warnings);
    if code_differs {
        let versions = |r: &Report| {
            r.plugins
                .iter()
                .filter_map(|p| Some((p.descriptor.clone(), p.version.clone()?)))
                .collect::<BTreeSet<_>>()
        };
        if let Some((_, v)) = versions(&old).intersection(&versions(&new)).next() {
            warnings.push(Warning {
                code: "diff.same-version".into(),
                location: String::new(),
                message: format!("both jars say they're version {v}, but their code differs"),
            });
        }
    }

    for (side, r) in [("old", &old), ("new", &new)] {
        if r.partial {
            warnings.push(Warning {
                code: "diff.partial".into(),
                location: String::new(),
                message: format!(
                    "the {side} report is partial, so this comparison may be missing changes"
                ),
            });
        }
    }

    added.sort_by_key(Item::sort_key);
    removed.sort_by_key(Item::sort_key);
    changed.sort();
    let side = |r: Report| Side {
        file: r.file,
        plugins: r.plugins,
        partial: r.partial,
        warnings: r.warnings,
    };
    Diff {
        schema_version: SCHEMA_VERSION,
        tool_version: env!("CARGO_PKG_VERSION").to_string(),
        obfuscation: OldNew {
            old: old.obfuscation.score,
            new: new.obfuscation.score,
        },
        old: side(old),
        new: side(new),
        added,
        removed,
        changed,
        code,
        same_code,
        unchanged,
        warnings,
        disclaimer: DISCLAIMER.to_string(),
    }
}

fn code_diff(old: &Classes, new: &Classes) -> CodeDiff {
    let mut code = CodeDiff::default();
    let only = |a: &BTreeMap<String, ClassPrint>, b: &BTreeMap<String, ClassPrint>| {
        a.keys().filter(|k| !b.contains_key(*k)).cloned().collect()
    };
    code.added_classes = only(&new.code, &old.code);
    code.removed_classes = only(&old.code, &new.code);
    for (class, b) in &new.code {
        let Some(a) = old.code.get(class) else {
            continue;
        };
        if a.header == b.header
            && a.methods == b.methods
            && a.fields == b.fields
            && a.duplicate == b.duplicate
        {
            code.unchanged_classes += 1;
            continue;
        }
        type Prints = BTreeMap<String, [u8; 32]>;
        let keys = |m: &Prints, other: &Prints| {
            m.keys()
                .filter(|k| !other.contains_key(*k))
                .cloned()
                .collect()
        };
        let changed = |m: &Prints, other: &Prints| {
            m.iter()
                .filter(|(k, v)| other.get(*k).is_some_and(|old| old != *v))
                .map(|(k, _)| k.clone())
                .collect()
        };
        let no_rules = BTreeSet::new();
        let rules_of =
            |c: &Classes, p: &ClassPrint| c.rules.get(&p.location).unwrap_or(&no_rules).clone();
        let rules_added: Vec<String> = rules_of(new, b)
            .difference(&rules_of(old, a))
            .map(|r| r.to_string())
            .collect();
        let duplicate = a.duplicate || b.duplicate;
        let severity = rules_added
            .iter()
            .map(|r| rules::rule(r).severity)
            .max()
            .unwrap_or(Severity::Info);
        code.changed_classes.push(ClassChange {
            class: class.clone(),
            // Which copy the server loads isn't something to leave at info.
            severity: if duplicate {
                severity.max(Severity::Notable)
            } else {
                severity
            },
            rules_added,
            header_changed: a.header != b.header && !duplicate,
            methods_added: keys(&b.methods, &a.methods),
            methods_removed: keys(&a.methods, &b.methods),
            methods_changed: changed(&b.methods, &a.methods),
            fields_added: keys(&b.fields, &a.fields),
            fields_removed: keys(&a.fields, &b.fields),
            fields_changed: changed(&b.fields, &a.fields),
            calls_added: b.calls.difference(&a.calls).cloned().collect(),
            duplicate,
        });
    }
    code.changed_classes.sort_by(|x, y| {
        y.severity
            .cmp(&x.severity)
            .then_with(|| x.class.cmp(&y.class))
    });

    let files = |c: &Classes| -> BTreeMap<String, [u8; 32]> {
        c.files
            .iter()
            .filter(|(p, _)| !build_noise(p))
            .map(|(p, h)| (p.clone(), *h))
            .collect()
    };
    let (a, b) = (files(old), files(new));
    code.added_files = b.keys().filter(|k| !a.contains_key(*k)).cloned().collect();
    code.removed_files = a.keys().filter(|k| !b.contains_key(*k)).cloned().collect();
    code.changed_files = b
        .iter()
        .filter(|(k, v)| a.get(*k).is_some_and(|old| old != *v))
        .map(|(k, _)| k.clone())
        .collect();
    code
}

/// Files a build or a re-signing rewrites without touching the code: jar signatures
/// (`META-INF/*.SF`, `.RSA`, `.DSA`, `.EC`) and Maven's `pom.properties`, with its build
/// date, and `pom.xml`.
fn build_noise(path: &str) -> bool {
    let p = path
        .rsplit("!/")
        .next()
        .unwrap_or(path)
        .to_ascii_uppercase();
    let Some(rest) = p.strip_prefix("META-INF/") else {
        return false;
    };
    (rest.starts_with("MAVEN/")
        && (rest.ends_with("/POM.PROPERTIES") || rest.ends_with("/POM.XML")))
        || (!rest.contains('/')
            && [".SF", ".RSA", ".DSA", ".EC"]
                .iter()
                .any(|e| rest.ends_with(e)))
}

fn maven_versions(r: &Report) -> Versions {
    let mut m = Versions::new();
    for coord in r.plugins.iter().flat_map(|p| &p.maven_libraries) {
        let (key, version) = match coord.rsplit_once(':') {
            Some((k, v)) if k.contains(':') => (k.to_string(), v.to_string()),
            _ => (coord.clone(), String::new()),
        };
        m.entry(key).or_default().insert(version);
    }
    m
}

fn bundled_versions(r: &Report) -> Versions {
    let mut m = Versions::new();
    for l in &r.libraries {
        m.entry(format!("{}:{}", l.group, l.artifact))
            .or_default()
            .insert(l.version.clone());
    }
    m
}

fn join(v: &BTreeSet<String>) -> String {
    v.iter().cloned().collect::<Vec<_>>().join(", ")
}
