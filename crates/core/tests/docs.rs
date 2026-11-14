//! The rule docs must match `RULES`: a heading, title, why and legit-uses line in docs/rules.md
//! and a row in the README rule table for every rule, and nothing for rules that don't exist.

use jarsnitch_core::RULES;

fn read(path: &str) -> String {
    let path = format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}")).replace("\r\n", "\n")
}

#[test]
fn rules_md_matches_rules() {
    let doc = read("docs/rules.md");
    for rule in RULES {
        let sev = rule.severity.label().to_lowercase();
        // A prefix, because reports can add to a title: "Code is obfuscated (score N/100)".
        let want = format!("### `{}` · {sev}\n\n**{}", rule.id, rule.title);
        assert!(doc.contains(&want), "docs/rules.md needs:\n{want}");
    }
    let headings = doc.lines().filter(|l| l.starts_with("### `")).count();
    assert_eq!(headings, RULES.len(), "docs/rules.md has headings for rules that don't exist");
}

#[test]
fn readme_lists_every_rule() {
    let doc = read("README.md");
    for rule in RULES {
        let sev = rule.severity.label().to_lowercase();
        let want = format!("| `{}` | {sev} | {} |\n", rule.id, rule.title);
        assert!(doc.contains(&want), "README.md rule table needs:\n{want}");
    }
    let rows = doc.lines().filter(|l| l.starts_with("| `") && l.contains("` | ")).count();
    assert_eq!(rows, RULES.len(), "README.md lists rules that don't exist");
}

/// Markdown to the plain text `RULES` holds: links keep their text, `code` and *emphasis* lose their marks.
fn plain(md: &str) -> String {
    let mut out = String::new();
    let mut rest = md;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find("](").and_then(|mid| after[mid..].find(')').map(|close| (mid, mid + close))) {
            Some((mid, close)) => {
                out.push_str(&after[..mid]);
                rest = &after[close + 1..];
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out.replace(['`', '*'], "")
}

#[test]
fn rules_md_copy_matches_rules() {
    let doc = read("docs/rules.md");
    for rule in RULES {
        let start = doc.find(&format!("### `{}` · ", rule.id)).expect("heading checked above");
        let section = &doc[start..];
        let end = section[4..].find("\n#").map_or(section.len(), |i| i + 4);
        let line = |label: &str| {
            let prefix = format!("- **{label}:** ");
            let found = section[..end].lines().find_map(|l| l.strip_prefix(prefix.as_str()));
            plain(found.unwrap_or_else(|| panic!("docs/rules.md: `{}` has no \"{label}\" line", rule.id)))
        };
        assert_eq!(line("Why it matters"), rule.why, "docs/rules.md: why it matters for `{}`", rule.id);
        assert_eq!(line("Legit uses"), rule.legit_uses, "docs/rules.md: legit uses for `{}`", rule.id);
    }
}
