//! The rule docs must match `RULES`: a heading and title in docs/rules.md and a row in
//! the README rule table for every rule, and nothing for rules that don't exist.

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
