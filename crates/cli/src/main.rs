mod check;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use check::{Checked, Source};
use jarsnitch_core::{
    Change, ClassChange, DISCLAIMER, Diff, Finding, Item, Limits, Report, Severity,
};

const USAGE: &str = "\
jarsnitch: see what a Minecraft plugin can do before you install it

usage:
  jarsnitch scan <jar|folder>... [--json]
  jarsnitch diff <old.jar> <new.jar> [--json]
  jarsnitch check <jar|folder>... [--mc <version>] [--no-updates] [--json]
  jarsnitch --version

A folder means every .jar directly inside it.
check compares each jar with the official release on Modrinth and Hangar and looks
for updates there. It's the only command that uses the network.
Exit codes: 0 no High findings (diff: none added; check: no jar's code differs from
its official file and nothing adds a High finding), 1 a file couldn't be read, a
request failed or there was nothing to scan, 2 at least one High finding (diff: the
update adds one; check: a jar's code differs from its official file, or the jar or
its update adds a High finding).";

/// Changed classes listed before "and N more", besides those with notable or High code.
const CLASS_LIST: usize = 10;

/// `println!` of one line, with control characters (line breaks too) shown as escapes, so
/// text from a jar or a website can't move the cursor, hide or add lines, or retitle the
/// terminal. Print a blank line with `say!()`.
macro_rules! say {
    () => { println!() };
    ($($t:tt)*) => { println!("{}", visible(&format!($($t)*))) };
}

/// `eprintln!`, made visible the same way.
macro_rules! esay {
    ($($t:tt)*) => { eprintln!("{}", visible(&format!($($t)*))) };
}

/// Control characters, and the ones that reorder or break text, which a terminal acts on.
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn visible(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains(hidden) {
        return s.into();
    }
    s.chars()
        .map(|c| {
            if hidden(c) {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect::<String>()
        .into()
}

/// JSON with the hidden characters serde_json leaves as they are (DEL, C1 controls, bidi
/// marks) written as `\uXXXX`. Those can only be inside strings, so the JSON means the same.
fn to_json(value: &impl serde::Serialize) -> serde_json::Result<String> {
    let s = serde_json::to_string_pretty(value)?;
    if !s.contains(|c: char| c >= '\u{7f}' && hidden(c)) {
        return Ok(s);
    }
    Ok(s.chars()
        .map(|c| {
            if c >= '\u{7f}' && hidden(c) {
                format!("\\u{:04x}", c as u32)
            } else {
                c.to_string()
            }
        })
        .collect())
}

/// A file the server or Java reads to decide what runs: a plugin descriptor, the
/// manifest or a service list.
fn runtime_file(path: &str) -> bool {
    const READ: [&str; 10] = [
        "plugin.yml",
        "paper-plugin.yml",
        "bungee.yml",
        "velocity-plugin.json",
        "fabric.mod.json",
        "quilt.mod.json",
        "META-INF/mods.toml",
        "META-INF/neoforge.mods.toml",
        "META-INF/MANIFEST.MF",
        "META-INF/coremods.json",
    ];
    let lower = path.to_ascii_lowercase();
    // Mixin configs pick the classes that rewrite other code as it loads.
    let mixins = lower.ends_with(".json")
        && (lower.ends_with(".mixins.json")
            || lower
                .rsplit('/')
                .next()
                .is_some_and(|f| f.starts_with("mixins.")));
    READ.iter().any(|r| path.eq_ignore_ascii_case(r))
        || path.starts_with("META-INF/services/")
        || mixins
}

/// The project's name or address contains the plugin's name, or the reverse.
fn named_like(plugin: &str, o: &Source) -> bool {
    let norm = |s: &str| {
        s.chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(char::to_lowercase)
            .collect::<String>()
    };
    let p = norm(plugin);
    let slug = o.url.rsplit('/').next().unwrap_or("");
    p.is_empty()
        || [o.project.as_str(), slug]
            .iter()
            .map(|s| norm(s))
            .any(|s| !s.is_empty() && (s.contains(&p) || p.contains(&s)))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") || args.first().is_some_and(|a| a == "help")
    {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        say!("jarsnitch {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let (mut json, mut updates, mut mc) = (false, true, None);
    let mut rest: Vec<&String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => json = true,
            "--no-updates" => updates = false,
            "--mc" => match it.next() {
                Some(v) => mc = Some(v.clone()),
                None => {
                    esay!("error: --mc needs a Minecraft version, like --mc 1.21.4");
                    eprintln!("\n{USAGE}");
                    return ExitCode::from(1);
                }
            },
            s if s.starts_with("--mc=") => mc = Some(s["--mc=".len()..].to_string()),
            s if s.starts_with('-') => {
                esay!("error: unknown option {s}");
                eprintln!("\n{USAGE}");
                return ExitCode::from(1);
            }
            _ => rest.push(a),
        }
    }
    let is_check = rest.first().is_some_and(|c| c.as_str() == "check");
    if !is_check && (mc.is_some() || !updates) {
        esay!("error: --mc and --no-updates only go with check");
        eprintln!("\n{USAGE}");
        return ExitCode::from(1);
    }
    match rest.split_first() {
        Some((cmd, paths)) if cmd.as_str() == "scan" && !paths.is_empty() => scan(paths, json),
        Some((cmd, [old, new])) if cmd.as_str() == "diff" => diff(old, new, json),
        Some((cmd, paths)) if cmd.as_str() == "check" && !paths.is_empty() => {
            check(paths, json, mc.as_deref(), updates)
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(1)
        }
    }
}

fn jar_files(arg: &str) -> Result<Vec<PathBuf>, String> {
    let path = Path::new(arg);
    if !path.is_dir() {
        return Ok(vec![path.to_path_buf()]);
    }
    let mut jars: Vec<PathBuf> = std::fs::read_dir(path)
        .map_err(|e| format!("{arg}: {e}"))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
        .collect();
    jars.sort();
    Ok(jars)
}

/// File name and bytes of every jar the arguments name. `failed` is set when one can't be read.
fn read_jars(args: &[&String], failed: &mut bool) -> Vec<(String, Vec<u8>)> {
    let mut jars = Vec::new();
    for arg in args {
        let files = match jar_files(arg) {
            Ok(f) => f,
            Err(e) => {
                esay!("error: {e}");
                *failed = true;
                continue;
            }
        };
        if files.is_empty() {
            esay!("warning: no .jar files in {arg}");
        }
        for file in files {
            let name = file.file_name().map_or_else(
                || file.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            match std::fs::read(&file) {
                Ok(bytes) => jars.push((name, bytes)),
                Err(e) => {
                    esay!("error: {}: {e}", file.display());
                    *failed = true;
                }
            }
        }
    }
    jars
}

fn scan(args: &[&String], json: bool) -> ExitCode {
    let limits = Limits::default();
    let mut failed = false;
    let mut reports = Vec::new();
    for (name, bytes) in read_jars(args, &mut failed) {
        let report = jarsnitch_core::scan(&name, &bytes, &limits);
        failed |= report.is_unreadable();
        reports.push(report);
    }

    // A CI job pointed at the wrong folder shouldn't pass.
    failed |= reports.is_empty();

    if json {
        match to_json(&reports) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                esay!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        for r in &reports {
            print_report(r);
        }
        if !reports.is_empty() {
            say!("{DISCLAIMER}");
        }
    }

    if reports.iter().any(Report::has_high) {
        ExitCode::from(2)
    } else if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn check(args: &[&String], json: bool, mc: Option<&str>, updates: bool) -> ExitCode {
    let mut failed = false;
    let jars = read_jars(args, &mut failed);
    if jars.is_empty() {
        return ExitCode::from(1);
    }
    esay!(
        "jarsnitch check: asking api.modrinth.com and hangar.papermc.io about {} jar{}. This sends their SHA-512 and SHA-256 hashes, plugin names and versions, and downloads official files to compare with. The jars themselves aren't sent.",
        jars.len(),
        if jars.len() == 1 { "" } else { "s" }
    );
    let results = check::run(&check::Net::new(), jars, mc, updates, &Limits::default());
    failed |= results
        .iter()
        .any(|c| !c.errors.is_empty() || c.status == "unreadable");
    if json {
        match to_json(&results) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                esay!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        for c in &results {
            print_check(c);
        }
        print_check_summary(&results);
    }
    if results.iter().any(Checked::alarming) {
        ExitCode::from(2)
    } else if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn site(s: &Source) -> &'static str {
    if s.site == "modrinth" {
        "Modrinth"
    } else {
        "Hangar"
    }
}

fn release(s: &Source) -> String {
    match &s.version {
        Some(v) => format!("{} {v} on {}", s.project, site(s)),
        None => format!("{} on {}", s.project, site(s)),
    }
}

fn print_check(c: &Checked) {
    say!("== {} ==", c.file);
    if let Some(p) = &c.plugin {
        say!("{p} {}", c.version.as_deref().unwrap_or("?"));
    }
    // Anyone can publish a copy as their own project.
    let named = |o: &Source| {
        if let Some(p) = &c.plugin
            && !named_like(p, o)
        {
            say!(
                "  That project isn't named like the plugin ({p}). Make sure it's the one you meant."
            );
        }
    };
    match (c.status, &c.official) {
        ("official", Some(o)) => {
            say!(
                "Official file: the same bytes as {} ({})",
                release(o),
                o.url
            );
            named(o);
        }
        ("same-code", Some(o)) => {
            say!(
                "Same code as {} ({}), but not the same file.",
                release(o),
                o.url
            );
            named(o);
            if let Some(d) = &o.diff {
                let c = &d.code;
                let files: Vec<String> = c
                    .changed_files
                    .iter()
                    .map(|f| format!("~ {f}"))
                    .chain(c.added_files.iter().map(|f| format!("+ {f}")))
                    .chain(c.removed_files.iter().map(|f| format!("- {f}")))
                    .collect();
                if files.is_empty() {
                    say!("  Every file is the same; only the zip around them differs.");
                } else {
                    say!("  Other files differ: {}", capped(&files, 8));
                    let read: Vec<String> = c
                        .changed_files
                        .iter()
                        .chain(&c.added_files)
                        .chain(&c.removed_files)
                        .filter(|f| runtime_file(f))
                        .cloned()
                        .collect();
                    if !read.is_empty() {
                        say!(
                            "  The server or Java reads {}: it sets commands, permissions or what loads, so compare it with the official file.",
                            capped(&read, 4)
                        );
                    }
                }
                let adds: Vec<&Item> = d
                    .added
                    .iter()
                    .filter(|i| i.severity() > Severity::Info)
                    .collect();
                if !adds.is_empty() {
                    say!("  This jar adds:");
                    for i in adds {
                        say!("    {:<8} {}", i.severity().label(), item_text(i, "New"));
                    }
                }
            }
        }
        ("differs", Some(o)) => {
            say!("CODE DIFFERS from {} ({})", release(o), o.url);
            named(o);
            if let Some(url) = &o.download {
                say!("  official file: {url}");
            }
            if let Some(d) = &o.diff {
                // That the versions match is what a check starts from.
                for w in d.warnings.iter().filter(|w| w.code != "diff.same-version") {
                    say!("WARNING: {}", w.message);
                }
                // A changed main class or startup library is a reason the code differs.
                if !d.changed.is_empty() {
                    say!("Changes:");
                    for ch in &d.changed {
                        print_change(ch);
                    }
                }
                say!();
                print_items("This jar ADDS", "New", &d.added);
                print_code(d);
            }
        }
        ("unreadable", _) => say!("Not a readable jar."),
        _ => match (&c.unlike, &c.plugin, &c.version) {
            (Some(u), Some(p), Some(v)) => {
                say!("Not found on Modrinth or Hangar: no file with these bytes.");
                say!(
                    "  {} ({}) has a {p} {v} that isn't this plugin: its name, or its main class and most of its classes, differ from this jar's.",
                    release(u),
                    u.url
                );
                if let Some(url) = &u.download {
                    say!("  Compare them with jarsnitch diff: {url}");
                }
            }
            (_, Some(p), Some(v)) => {
                say!(
                    "Not found on Modrinth or Hangar: no file with these bytes, and no official {p} {v} to compare with."
                );
                say!("  Paid, private and custom builds can't be checked this way.");
            }
            _ => {
                say!("Not found on Modrinth or Hangar: no file with these bytes.");
                say!("  Paid, private and custom builds can't be checked this way.");
            }
        },
    }
    if let Some(u) = &c.update {
        let date = u
            .published
            .as_deref()
            .map(|d| format!(", {}", d.get(..10).unwrap_or(d)))
            .unwrap_or_default();
        say!(
            "Update: {}{date} ({})",
            u.version.as_deref().unwrap_or("?"),
            u.url
        );
        if let Some(d) = &u.diff {
            // Findings, like the summary and the exit code; new classes with findings the
            // jar already has are listed after.
            let count = |s: Severity| {
                d.added
                    .iter()
                    .filter(|i| matches!(i, Item::Finding { finding } if finding.severity == s))
                    .count()
            };
            let (high, notable) = (count(Severity::High), count(Severity::Notable));
            let items: Vec<&Item> = d
                .added
                .iter()
                .filter(|i| i.severity() > Severity::Info)
                .collect();
            if d.old.partial || d.new.partial {
                say!("  incomplete comparison: findings below cover only the scanned contents");
            } else {
                match (high + notable, items.is_empty()) {
                    (0, true) => say!("  adds no notable or High findings"),
                    (0, false) => {
                        say!("  adds no notable or High findings, but new classes with them:")
                    }
                    _ => say!("  adds {high} High and {notable} notable findings:"),
                }
            }
            for i in items {
                say!("    {:<8} {}", i.severity().label(), item_text(i, "New"));
            }
            let c = &d.code;
            say!(
                "  code: {} classes changed, {} added, {} removed",
                c.changed_classes.len(),
                c.added_classes.len(),
                c.removed_classes.len()
            );
        } else {
            say!("  not compared: that release has no file for this server software");
        }
    }
    for e in &c.errors {
        say!("  couldn't check: {e}");
    }
    say!();
}

fn print_check_summary(results: &[Checked]) {
    let n = |s: &str| results.iter().filter(|c| c.status == s).count();
    let updates = results.iter().filter(|c| c.update.is_some()).count();
    let adds = |d: Option<&Diff>| d.is_some_and(Diff::adds_high);
    let high = |k: usize| match k {
        0 => String::new(),
        1 => " (1 adds a High finding)".into(),
        k => format!(" ({k} add a High finding)"),
    };
    let same_high = results
        .iter()
        .filter(|c| {
            c.status != "differs" && adds(c.official.as_ref().and_then(|o| o.diff.as_ref()))
        })
        .count();
    let update_high = results
        .iter()
        .filter(|c| adds(c.update.as_ref().and_then(|u| u.diff.as_ref())))
        .count();
    say!(
        "{} jar{} · {} official · {} same code{} · {} code differs · {} not found · {updates} update{} available{}",
        results.len(),
        if results.len() == 1 { "" } else { "s" },
        n("official"),
        n("same-code"),
        high(same_high),
        n("differs"),
        n("not-found"),
        if updates == 1 { "" } else { "s" },
        high(update_high),
    );
    if n("official") + n("same-code") > 0 {
        say!(
            "Official means the project published that file; it isn't a check of what the code does. jarsnitch scan shows that."
        );
    }
}

/// The first `max` entries joined, then how many more.
fn capped(items: &[String], max: usize) -> String {
    let mut s = items
        .iter()
        .take(max)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if items.len() > max {
        s.push_str(&format!(", +{} more", items.len() - max));
    }
    s
}

fn read(path: &str) -> Option<(String, Vec<u8>)> {
    let p = Path::new(path);
    let name = p
        .file_name()
        .map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
    match std::fs::read(p) {
        Ok(bytes) => Some((name, bytes)),
        Err(e) => {
            esay!("error: {path}: {e}");
            None
        }
    }
}

fn diff(old: &str, new: &str, json: bool) -> ExitCode {
    let (Some((old_name, old_bytes)), Some((new_name, new_bytes))) = (read(old), read(new)) else {
        return ExitCode::from(1);
    };
    let d = jarsnitch_core::diff(
        &old_name,
        &old_bytes,
        &new_name,
        &new_bytes,
        &Limits::default(),
    );
    if d.is_unreadable() {
        for (path, side) in [(old, &d.old), (new, &d.new)] {
            if let Some(w) = side
                .warnings
                .iter()
                .find(|w| w.code == "zip.unreadable" && w.location.is_empty())
            {
                esay!("error: {path}: not a readable jar ({})", w.message);
            }
        }
        return ExitCode::from(1);
    }
    if json {
        match to_json(&d) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                esay!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        print_diff(&d);
    }
    if d.adds_high() {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

fn print_diff(d: &Diff) {
    say!("== {} -> {} ==", d.old.file.name, d.new.file.name);
    let plugin = |s: &jarsnitch_core::Side| {
        s.plugins.first().map(|p| {
            (
                p.name.clone().unwrap_or_else(|| "(unnamed)".into()),
                p.version.clone().unwrap_or_else(|| "?".into()),
            )
        })
    };
    match (plugin(&d.old), plugin(&d.new)) {
        (Some((a, av)), Some((b, bv))) if a == b => say!("{a} {av} -> {bv}"),
        (Some((a, av)), Some((b, bv))) => say!("{a} {av} -> {b} {bv}"),
        _ => {}
    }
    say!(
        "sha256 {} -> {} · obfuscation {} -> {}",
        &d.old.file.sha256[..12],
        &d.new.file.sha256[..12],
        d.obfuscation.old,
        d.obfuscation.new
    );
    for w in &d.warnings {
        say!("WARNING: {}", w.message);
    }
    say!();

    print_items("This update ADDS", "New", &d.added);
    print_items("This update REMOVES", "Removed", &d.removed);

    if !d.changed.is_empty() {
        say!("Changes:");
        for c in &d.changed {
            print_change(c);
        }
        if d.changed.iter().any(|c| {
            c.kind == "plugin-field" && (c.key.ends_with(" authors") || c.key.ends_with(" website"))
        }) {
            say!("A different author or website can mean the plugin has a new owner.");
        }
        say!();
    }
    print_code(d);
    let mut unchanged: Vec<(&str, usize)> = Vec::new();
    for k in &d.unchanged {
        match unchanged.last_mut() {
            Some((rule, n)) if *rule == k.rule => *n += 1,
            _ => unchanged.push((&k.rule, 1)),
        }
    }
    if !unchanged.is_empty() {
        let list: Vec<String> = unchanged
            .iter()
            .map(|(r, n)| {
                if *n > 1 {
                    format!("{r} ({n})")
                } else {
                    r.to_string()
                }
            })
            .collect();
        say!("Unchanged: {}", list.join(", "));
        say!();
    }
    say!("{DISCLAIMER}");
}

fn item_text(item: &Item, verb: &str) -> String {
    match item {
        Item::Finding { finding } => match &finding.value {
            Some(v) => format!("{}: {v}   [{}]", finding.title, finding.rule),
            None => format!("{}   [{}]", finding.title, finding.rule),
        },
        Item::Class { value, rules, .. } => {
            format!("{verb} class: {value}   [{}]", rules.join(", "))
        }
        Item::Command { value } => format!("{verb} command: /{value}"),
        Item::Permission { value } => format!("{verb} permission: {value}"),
        Item::MavenLibrary { value } => format!("{verb} downloaded library: {value}"),
        Item::Library { value } => format!("{verb} bundled library: {value}"),
        Item::Dependency { value } => format!("{verb} dependency: {value}"),
        Item::OptionalDependency { value } => format!("{verb} optional dependency: {value}"),
    }
}

fn print_items(heading: &str, verb: &str, items: &[Item]) {
    if items.is_empty() {
        say!("{heading}: nothing");
        say!();
        return;
    }
    say!("{heading}:");
    say!();
    for item in items {
        match item {
            Item::Finding { finding } => print_finding(finding),
            _ => {
                say!("{:<8} {}", item.severity().label(), item_text(item, verb));
                say!();
            }
        }
    }
}

/// The class-by-class comparison: changed classes with notable or High code first.
fn print_code(d: &Diff) {
    if d.warnings.iter().any(|w| w.code == "diff.classes-renamed") {
        return;
    }
    let c = &d.code;
    let classes = |n: usize| format!("{n} class{}", if n == 1 { "" } else { "es" });
    if d.same_code() {
        say!(
            "Code: the same in both ({}).",
            classes(c.unchanged_classes as usize)
        );
    } else {
        say!(
            "Code: {} changed, {} added, {} removed, {} unchanged",
            classes(c.changed_classes.len()),
            c.added_classes.len(),
            c.removed_classes.len(),
            c.unchanged_classes
        );
        say!();
        let notable = c
            .changed_classes
            .iter()
            .filter(|x| x.severity > Severity::Info)
            .count();
        let shown = notable.max(CLASS_LIST.min(c.changed_classes.len()));
        for x in &c.changed_classes[..shown] {
            print_class_change(x);
        }
        if c.changed_classes.len() > shown {
            say!(
                "         and {} more changed classes (--json lists them all)",
                c.changed_classes.len() - shown
            );
            say!();
        }
        for (what, list) in [
            ("New classes", &c.added_classes),
            ("Removed classes", &c.removed_classes),
        ] {
            if !list.is_empty() {
                say!("{what} ({}): {}", list.len(), capped(list, 8));
            }
        }
    }
    let files: Vec<String> = c
        .added_files
        .iter()
        .map(|f| format!("+ {f}"))
        .chain(c.removed_files.iter().map(|f| format!("- {f}")))
        .chain(c.changed_files.iter().map(|f| format!("~ {f}")))
        .collect();
    if !files.is_empty() {
        say!("Other files: {}", capped(&files, 8));
    }
    // A changed class ends with a blank line of its own.
    if d.same_code()
        || !files.is_empty()
        || !(c.added_classes.is_empty() && c.removed_classes.is_empty())
    {
        say!();
    }
}

fn print_class_change(x: &ClassChange) {
    let rules = if x.rules_added.is_empty() {
        String::new()
    } else {
        format!("   [{}]", x.rules_added.join(", "))
    };
    say!(
        "{:<8} Changed class: {}{rules}",
        x.severity.label(),
        x.class
    );
    if x.duplicate {
        say!(
            "         ~ more than one entry has this name, with different code; the server may load any of them, so they're compared together"
        );
    }
    if x.header_changed {
        say!("         ~ superclass, interfaces, access flags or annotations");
    }
    let lines: Vec<String> = [
        ("+ method", &x.methods_added),
        ("- method", &x.methods_removed),
        ("~ method", &x.methods_changed),
        ("+ field", &x.fields_added),
        ("- field", &x.fields_removed),
        ("~ field", &x.fields_changed),
    ]
    .iter()
    .flat_map(|(mark, list)| list.iter().map(move |m| format!("{mark} {m}")))
    .collect();
    for l in lines.iter().take(8) {
        say!("         {l}");
    }
    if lines.len() > 8 {
        say!("         +{} more", lines.len() - 8);
    }
    if !x.calls_added.is_empty() {
        say!("         now calls: {}", capped(&x.calls_added, 6));
    }
    say!();
}

fn print_change(c: &Change) {
    let show = |v: &Option<String>| match v {
        None => "(none)".to_string(),
        Some(h) if c.kind == "nested" => h.chars().take(12).collect(),
        Some(v) => v.clone(),
    };
    let what = match c.kind {
        "library" => format!("bundled library {}", c.key),
        "maven-library" => format!("downloaded library {}", c.key),
        "nested" => format!("nested jar {} sha256", c.key),
        _ => c.key.clone(),
    };
    say!("  {what}: {} -> {}", show(&c.old), show(&c.new));
}

fn print_report(r: &Report) {
    say!("== {} ==", r.file.name);
    for p in &r.plugins {
        let name = p.name.as_deref().unwrap_or("(unnamed)");
        let version = p.version.as_deref().unwrap_or("?");
        say!("{name} {version} · {} ({})", p.platform, p.descriptor);
    }
    say!(
        "sha256 {} · {} class{} · obfuscation {}/100",
        &r.file.sha256[..12],
        r.stats.classes,
        if r.stats.classes == 1 { "" } else { "es" },
        r.obfuscation.score
    );
    if let Some(reach) = &r.reachability {
        let from = reach.entry_points.first().map_or("?", String::as_str);
        let more = match reach.entry_points.len() {
            0 | 1 => String::new(),
            n => format!(" and {} more", n - 1),
        };
        say!(
            "call graph from {from}{more}: {} of {} methods reached",
            reach.reached_methods,
            reach.methods
        );
        if reach.dynamic {
            say!("  the plugin also loads code at runtime, so code it doesn't reach may still run");
        }
    }
    if r.partial {
        say!(
            "PARTIAL REPORT: a scan limit was hit, so some of the jar wasn't checked. See warnings."
        );
    }
    say!();
    // Code the entry points never call goes last; its severity stays the same.
    let (unreached, reached): (Vec<&Finding>, Vec<&Finding>) =
        r.findings.iter().partition(|f| f.reached == Some(false));
    for f in reached.iter().chain(&unreached) {
        print_finding(f);
    }
    if !r.warnings.is_empty() {
        say!("Warnings:");
        for w in &r.warnings {
            let at = if w.location.is_empty() {
                String::new()
            } else {
                format!("{}: ", w.location)
            };
            say!("  {:<22} {at}{}", w.code, w.message);
        }
        say!();
    }
    let count = |s| r.findings.iter().filter(|f| f.severity == s).count();
    let not_called = match unreached.len() {
        0 => String::new(),
        n => format!(" · {n} not called from entry points"),
    };
    say!(
        "{} high · {} notable · {} info{not_called}",
        count(Severity::High),
        count(Severity::Notable),
        count(Severity::Info)
    );
    say!();
}

fn print_finding(f: &Finding) {
    let title = match &f.value {
        Some(v) => format!("{}: {v}", f.title),
        None => f.title.clone(),
    };
    say!("{:<8} {title}   [{}]", f.severity.label(), f.rule);
    say!("         {}", f.why);
    let mut places: Vec<String> = f
        .evidence
        .iter()
        .take(3)
        .map(|e| {
            let loc = if e.location.is_empty() {
                "(jar)"
            } else {
                &e.location
            };
            match &e.method {
                Some(m) => format!("{loc}.{}", m.split('(').next().unwrap_or(m)),
                None => loc.to_string(),
            }
        })
        .collect();
    places.dedup();
    let more = f.evidence_total.saturating_sub(3);
    let tail = if more > 0 {
        format!(", +{more} more")
    } else {
        String::new()
    };
    say!("         at {}{tail}", places.join(", "));
    if f.reached == Some(false) {
        say!("         not called from the plugin's entry points");
    }
    say!();
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_hides_terminal_controls() {
        let s = super::to_json(&["A\u{9b}31mB\u{202e}C\u{7f}\n"]).unwrap();
        assert_eq!(s, "[\n  \"A\\u009b31mB\\u202eC\\u007f\\n\"\n]");
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&s).unwrap()[0],
            "A\u{9b}31mB\u{202e}C\u{7f}\n"
        );
    }

    #[test]
    fn runtime_files() {
        for f in [
            "plugin.yml",
            "META-INF/MANIFEST.MF",
            "META-INF/services/x.Y",
            "META-INF/coremods.json",
            "fixture.mixins.json",
            "a/mixins.fixture.json",
        ] {
            assert!(super::runtime_file(f), "{f}");
        }
        for f in ["config.yml", "lang/mixins.txt", "data.json"] {
            assert!(!super::runtime_file(f), "{f}");
        }
    }

    #[test]
    fn escapes_terminal_controls() {
        assert_eq!(
            super::visible("2.0.0\nOfficial file: forged"),
            "2.0.0\\u{a}Official file: forged"
        );
        assert_eq!(
            super::visible("\u{1b}]0;title\u{7}"),
            "\\u{1b}]0;title\\u{7}"
        );
        assert_eq!(super::visible("x\u{202e}y\ru"), "x\\u{202e}y\\u{d}u");
    }
}
