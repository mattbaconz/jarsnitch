use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jarsnitch_core::{Change, Diff, Finding, Item, Limits, Report, Severity, DISCLAIMER};

const USAGE: &str = "\
jarsnitch: see what a Minecraft plugin can do before you install it

usage:
  jarsnitch scan <jar|folder>... [--json]
  jarsnitch diff <old.jar> <new.jar> [--json]
  jarsnitch --version

A folder scans every .jar directly inside it.
Exit codes: 0 no High findings (diff: none added), 1 a file couldn't be read or
there was nothing to scan, 2 at least one High finding (diff: the update adds one).";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") || args.first().is_some_and(|a| a == "help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "-V" || a == "--version") {
        println!("jarsnitch {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let json = args.iter().any(|a| a == "--json");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--json").collect();
    if let Some(flag) = rest.iter().find(|a| a.starts_with('-')) {
        eprintln!("error: unknown option {flag}\n\n{USAGE}");
        return ExitCode::from(1);
    }
    match rest.split_first() {
        Some((cmd, paths)) if cmd.as_str() == "scan" && !paths.is_empty() => scan(paths, json),
        Some((cmd, [old, new])) if cmd.as_str() == "diff" => diff(old, new, json),
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

fn scan(args: &[&String], json: bool) -> ExitCode {
    let limits = Limits::default();
    let mut reports = Vec::new();
    let mut failed = false;
    for arg in args {
        let files = match jar_files(arg) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: {e}");
                failed = true;
                continue;
            }
        };
        if files.is_empty() {
            eprintln!("warning: no .jar files in {arg}");
        }
        for file in files {
            let name = file.file_name().map_or_else(|| file.display().to_string(), |n| n.to_string_lossy().into_owned());
            match std::fs::read(&file) {
                Ok(bytes) => {
                    let report = jarsnitch_core::scan(&name, &bytes, &limits);
                    failed |= report.is_unreadable();
                    reports.push(report);
                }
                Err(e) => {
                    eprintln!("error: {}: {e}", file.display());
                    failed = true;
                }
            }
        }
    }

    // A CI job pointed at the wrong folder shouldn't pass.
    failed |= reports.is_empty();

    if json {
        match serde_json::to_string_pretty(&reports) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        for r in &reports {
            print_report(r);
        }
        if !reports.is_empty() {
            println!("{DISCLAIMER}");
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

fn read(path: &str) -> Option<(String, Vec<u8>)> {
    let p = Path::new(path);
    let name = p.file_name().map_or_else(|| path.to_string(), |n| n.to_string_lossy().into_owned());
    match std::fs::read(p) {
        Ok(bytes) => Some((name, bytes)),
        Err(e) => {
            eprintln!("error: {path}: {e}");
            None
        }
    }
}

fn diff(old: &str, new: &str, json: bool) -> ExitCode {
    let (Some((old_name, old_bytes)), Some((new_name, new_bytes))) = (read(old), read(new)) else {
        return ExitCode::from(1);
    };
    let d = jarsnitch_core::diff(&old_name, &old_bytes, &new_name, &new_bytes, &Limits::default());
    if d.is_unreadable() {
        for (path, side) in [(old, &d.old), (new, &d.new)] {
            if let Some(w) = side.warnings.iter().find(|w| w.code == "zip.unreadable" && w.location.is_empty()) {
                eprintln!("error: {path}: not a readable jar ({})", w.message);
            }
        }
        return ExitCode::from(1);
    }
    if json {
        match serde_json::to_string_pretty(&d) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(1);
            }
        }
    } else {
        print_diff(&d);
    }
    if d.adds_high() { ExitCode::from(2) } else { ExitCode::SUCCESS }
}

fn print_diff(d: &Diff) {
    println!("== {} -> {} ==", d.old.file.name, d.new.file.name);
    let plugin = |s: &jarsnitch_core::Side| s.plugins.first().map(|p| (p.name.clone().unwrap_or_else(|| "(unnamed)".into()), p.version.clone().unwrap_or_else(|| "?".into())));
    match (plugin(&d.old), plugin(&d.new)) {
        (Some((a, av)), Some((b, bv))) if a == b => println!("{a} {av} -> {bv}"),
        (Some((a, av)), Some((b, bv))) => println!("{a} {av} -> {b} {bv}"),
        _ => {}
    }
    println!(
        "sha256 {} -> {} · obfuscation {} -> {}",
        &d.old.file.sha256[..12],
        &d.new.file.sha256[..12],
        d.obfuscation.old,
        d.obfuscation.new
    );
    for w in &d.warnings {
        println!("WARNING: {}", w.message);
    }
    println!();

    for (heading, verb, items) in [("ADDS", "New", &d.added), ("REMOVES", "Removed", &d.removed)] {
        if items.is_empty() {
            println!("This update {heading}: nothing
");
            continue;
        }
        println!("This update {heading}:
");
        for item in items {
            match item {
                Item::Finding { finding } => print_finding(finding),
                Item::Class { value, rules, .. } => println!("{:<8} {verb} class: {value}   [{}]
", item.severity().label(), rules.join(", ")),
                Item::Command { value } => println!("{:<8} {verb} command: /{value}
", item.severity().label()),
                Item::Permission { value } => println!("{:<8} {verb} permission: {value}
", item.severity().label()),
                Item::MavenLibrary { value } => println!("{:<8} {verb} downloaded library: {value}
", item.severity().label()),
                Item::Library { value } => println!("{:<8} {verb} bundled library: {value}
", item.severity().label()),
            }
        }
    }

    if !d.changed.is_empty() {
        println!("Changes:");
        for c in &d.changed {
            print_change(c);
        }
        println!();
    }
    let mut unchanged: Vec<(&str, usize)> = Vec::new();
    for k in &d.unchanged {
        match unchanged.last_mut() {
            Some((rule, n)) if *rule == k.rule => *n += 1,
            _ => unchanged.push((&k.rule, 1)),
        }
    }
    if !unchanged.is_empty() {
        let list: Vec<String> = unchanged.iter().map(|(r, n)| if *n > 1 { format!("{r} ({n})") } else { r.to_string() }).collect();
        println!("Unchanged: {}
", list.join(", "));
    }
    println!("{DISCLAIMER}");
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
    println!("  {what}: {} -> {}", show(&c.old), show(&c.new));
}

fn print_report(r: &Report) {
    println!("== {} ==", r.file.name);
    for p in &r.plugins {
        let name = p.name.as_deref().unwrap_or("(unnamed)");
        let version = p.version.as_deref().unwrap_or("?");
        println!("{name} {version} · {} ({})", p.platform, p.descriptor);
    }
    println!(
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
        println!("call graph from {from}{more}: {} of {} methods reached", reach.reached_methods, reach.methods);
        if reach.dynamic {
            println!("  the plugin also loads code at runtime, so code it doesn't reach may still run");
        }
    }
    if r.partial {
        println!("PARTIAL REPORT: a scan limit was hit, so some of the jar wasn't checked. See warnings.");
    }
    println!();
    // Code the entry points never call goes last; its severity stays the same.
    let (unreached, reached): (Vec<&Finding>, Vec<&Finding>) = r.findings.iter().partition(|f| f.reached == Some(false));
    for f in reached.iter().chain(&unreached) {
        print_finding(f);
    }
    if !r.warnings.is_empty() {
        println!("Warnings:");
        for w in &r.warnings {
            let at = if w.location.is_empty() { String::new() } else { format!("{}: ", w.location) };
            println!("  {:<22} {at}{}", w.code, w.message);
        }
        println!();
    }
    let count = |s| r.findings.iter().filter(|f| f.severity == s).count();
    let not_called = match unreached.len() {
        0 => String::new(),
        n => format!(" · {n} not called from entry points"),
    };
    println!(
        "{} high · {} notable · {} info{not_called}\n",
        count(Severity::High),
        count(Severity::Notable),
        count(Severity::Info)
    );
}

fn print_finding(f: &Finding) {
    let title = match &f.value {
        Some(v) => format!("{}: {v}", f.title),
        None => f.title.clone(),
    };
    println!("{:<8} {title}   [{}]", f.severity.label(), f.rule);
    println!("         {}", f.why);
    let mut places: Vec<String> = f
        .evidence
        .iter()
        .take(3)
        .map(|e| {
            let loc = if e.location.is_empty() { "(jar)" } else { &e.location };
            match &e.method {
                Some(m) => format!("{loc}.{}", m.split('(').next().unwrap_or(m)),
                None => loc.to_string(),
            }
        })
        .collect();
    places.dedup();
    let more = f.evidence_total.saturating_sub(3);
    let tail = if more > 0 { format!(", +{more} more") } else { String::new() };
    println!("         at {}{tail}", places.join(", "));
    if f.reached == Some(false) {
        println!("         not called from the plugin's entry points");
    }
    println!();
}
