use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jarsnitch_core::{Finding, Limits, Report, Severity, DISCLAIMER};

const USAGE: &str = "\
jarsnitch: see what a Minecraft plugin can do before you install it

usage:
  jarsnitch scan <jar|folder>... [--json]

A folder scans every .jar directly inside it.
Exit codes: 0 no High findings, 1 a file couldn't be read, 2 at least one High finding.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json = args.iter().any(|a| a == "--json");
    let rest: Vec<&String> = args.iter().filter(|a| *a != "--json").collect();
    match rest.split_first() {
        Some((cmd, paths)) if cmd.as_str() == "scan" && !paths.is_empty() => scan(paths, json),
        Some((cmd, _)) if matches!(cmd.as_str(), "-h" | "--help" | "help") => {
            println!("{USAGE}");
            ExitCode::SUCCESS
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
    if r.partial {
        println!("PARTIAL REPORT: a scan limit was hit, so some of the jar wasn't checked. See warnings.");
    }
    println!();
    for f in &r.findings {
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
    println!(
        "{} high · {} notable · {} info\n",
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
    println!("         at {}{tail}\n", places.join(", "));
}
