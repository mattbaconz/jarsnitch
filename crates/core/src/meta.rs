//! Plugin descriptors, bundled-library metadata and the jar manifest.
//!
//! The YAML reader handles only the subset plugin descriptors use (top-level scalars and
//! lists, the keys of `commands:` / `permissions:`, Paper's `dependencies:`). It never
//! expands aliases, so a small file can't blow up into a huge document.

use crate::PluginInfo;

pub fn platform_for(path: &str) -> Option<&'static str> {
    Some(match path {
        "plugin.yml" => "bukkit",
        "paper-plugin.yml" => "paper",
        "bungee.yml" => "bungee",
        "velocity-plugin.json" => "velocity",
        "fabric.mod.json" => "fabric",
        "quilt.mod.json" => "quilt",
        "META-INF/mods.toml" => "forge",
        "META-INF/neoforge.mods.toml" => "neoforge",
        _ => return None,
    })
}

/// Parses a descriptor. `None` means it couldn't be read at all.
pub fn descriptor(path: &str, text: &str) -> Option<PluginInfo> {
    let platform = platform_for(path)?;
    let mut info = match platform {
        // YAML includes JSON, and some build tools write descriptors that way.
        "bukkit" | "paper" | "bungee" => match serde_json::from_str::<serde_json::Value>(text) {
            Ok(serde_json::Value::Object(map)) => json_descriptor(&map),
            _ => yaml_descriptor(text),
        },
        "velocity" => {
            let v: serde_json::Value = serde_json::from_str(text).ok()?;
            let deps = |optional: bool| -> Vec<String> {
                let all = v["dependencies"].as_array().into_iter().flatten();
                all.filter(|d| d["optional"].as_bool().unwrap_or(false) == optional).filter_map(|d| d["id"].as_str().map(str::to_string)).collect()
            };
            PluginInfo {
                name: json_str(&v, &["name"]).or_else(|| json_str(&v, &["id"])),
                version: json_str(&v, &["version"]),
                main: json_str(&v, &["main"]),
                authors: v["authors"].as_array().into_iter().flatten().filter_map(|a| a.as_str().map(str::to_string)).collect(),
                website: json_str(&v, &["url"]),
                dependencies: deps(false),
                optional_dependencies: deps(true),
                ..Default::default()
            }
        }
        "fabric" => {
            let v: serde_json::Value = serde_json::from_str(text).ok()?;
            PluginInfo {
                name: json_str(&v, &["name"]).or_else(|| json_str(&v, &["id"])),
                version: json_str(&v, &["version"]),
                ..Default::default()
            }
        }
        "quilt" => {
            let v: serde_json::Value = serde_json::from_str(text).ok()?;
            PluginInfo {
                name: json_str(&v, &["quilt_loader", "metadata", "name"])
                    .or_else(|| json_str(&v, &["quilt_loader", "id"])),
                version: json_str(&v, &["quilt_loader", "version"]),
                ..Default::default()
            }
        }
        _ => PluginInfo {
            name: toml_str(text, "displayName").or_else(|| toml_str(text, "modId")),
            version: toml_str(text, "version"),
            ..Default::default()
        },
    };
    info.platform = platform.to_string();
    info.descriptor = path.to_string();
    Some(info)
}

fn json_str(v: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for key in path {
        cur = cur.get(key)?;
    }
    cur.as_str().map(str::to_string)
}

/// First `key = "value"` line in a TOML file.
fn toml_str(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == key).then(|| unquote(v.trim()))
    })
}

fn strip_comment(line: &str) -> &str {
    let mut quote = None;
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '#') if prev_space => return &line[..i],
            _ => {}
        }
        prev_space = c.is_whitespace();
    }
    line
}

/// Splits `key: value` at the first colon outside quotes that ends the key.
fn split_kv(t: &str) -> Option<(String, &str)> {
    let mut quote = None;
    let b = t.as_bytes();
    for (i, c) in t.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') if i == 0 => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, ':') if i + 1 == b.len() || b[i + 1] == b' ' || b[i + 1] == b'\t' => {
                return Some((unquote(t[..i].trim()), t[i + 1..].trim()));
            }
            _ => {}
        }
    }
    None
}

fn unquote(s: &str) -> String {
    if s.len() >= 2 && s.starts_with('\'') && s.ends_with('\'') {
        s[1..s.len() - 1].replace("''", "'")
    } else if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn flow_list(v: &str) -> Vec<String> {
    v.trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| unquote(s.trim()))
        .filter(|s| !s.is_empty())
        .collect()
}

fn some(s: String) -> Option<String> {
    (!s.is_empty()).then_some(s)
}

fn add(list: &mut Vec<String>, item: String) {
    if !item.is_empty() && !list.contains(&item) {
        list.push(item);
    }
}

/// The list a top-level key fills: Bukkit and Paper `depend` / `softdepend`, BungeeCord
/// `depends` / `softDepends`.
fn list_field<'a>(d: &'a mut PluginInfo, key: &str) -> Option<&'a mut Vec<String>> {
    Some(match key {
        "authors" => &mut d.authors,
        "depend" | "depends" => &mut d.dependencies,
        "softdepend" | "softDepends" => &mut d.optional_dependencies,
        "libraries" => &mut d.maven_libraries,
        _ => return None,
    })
}

pub fn yaml_descriptor(text: &str) -> PluginInfo {
    let mut d = PluginInfo::default();
    // The keys that lead to the current line, outermost first, with their indents.
    let mut path: Vec<(usize, String)> = Vec::new();
    // Paper's `dependencies:` entries and whether each is required (the default).
    let mut paper: Vec<(String, bool)> = Vec::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim_end();
        let t = line.trim_start();
        if t.is_empty() || t == "---" || t == "..." {
            continue;
        }
        let indent = line.len() - t.len();
        // A list item belongs to the key above it, which may sit at the same indent.
        if let Some(item) = t.strip_prefix('-').filter(|r| r.is_empty() || r.starts_with([' ', '\t'])) {
            path.retain(|(i, _)| *i <= indent);
            if let [(_, key)] = path.as_slice()
                && let Some(list) = list_field(&mut d, key)
            {
                add(list, unquote(item.trim()));
            }
            continue;
        }
        path.retain(|(i, _)| *i < indent);
        let Some((key, value)) = split_kv(t) else { continue };
        let parents: Vec<&str> = path.iter().map(|(_, k)| k.as_str()).collect();
        match (parents.as_slice(), key.as_str()) {
            ([], "name") => d.name = some(unquote(value)),
            ([], "version") => d.version = some(unquote(value)),
            ([], "main") => d.main = some(unquote(value)),
            ([], "bootstrapper") => d.bootstrapper = some(unquote(value)),
            ([], "loader") => d.loader = some(unquote(value)),
            ([], "author") => add(&mut d.authors, unquote(value)),
            ([], "website") => d.website = some(unquote(value)),
            ([], "folia-supported") => d.folia_supported = unquote(value).eq_ignore_ascii_case("true"),
            ([], _) if !value.is_empty() => {
                if let Some(list) = list_field(&mut d, &key) {
                    flow_list(value).into_iter().for_each(|item| add(list, item));
                }
            }
            (["commands"], _) => d.commands.push(key.clone()),
            (["permissions"], _) => d.permissions.push(key.clone()),
            (["dependencies", "server" | "bootstrap"], _) => paper.push((key.clone(), true)),
            (["dependencies", "server" | "bootstrap", _], "required") => {
                if let Some(last) = paper.last_mut() {
                    last.1 = !unquote(value).eq_ignore_ascii_case("false");
                }
            }
            _ => {}
        }
        path.push((indent, key));
    }
    paper_dependencies(&mut d, paper);
    d
}

/// A plugin listed in both `server` and `bootstrap` is required if either says so.
fn paper_dependencies(d: &mut PluginInfo, paper: Vec<(String, bool)>) {
    for (name, _) in paper.iter().filter(|(_, required)| *required) {
        add(&mut d.dependencies, name.clone());
    }
    for (name, _) in paper {
        if !d.dependencies.contains(&name) {
            add(&mut d.optional_dependencies, name);
        }
    }
}

/// The same keys as [`yaml_descriptor`], from a descriptor written as JSON.
fn json_descriptor(map: &serde_json::Map<String, serde_json::Value>) -> PluginInfo {
    use serde_json::Value;
    let text = |v: &Value| match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => String::new(),
    };
    let keys = |v: &Value| v.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let mut d = PluginInfo::default();
    let mut paper = Vec::new();
    for (key, value) in map {
        match key.as_str() {
            "name" => d.name = some(text(value)),
            "version" => d.version = some(text(value)),
            "main" => d.main = some(text(value)),
            "bootstrapper" => d.bootstrapper = some(text(value)),
            "loader" => d.loader = some(text(value)),
            "author" => add(&mut d.authors, text(value)),
            "website" => d.website = some(text(value)),
            "folia-supported" => d.folia_supported = text(value).eq_ignore_ascii_case("true"),
            "commands" => d.commands = keys(value),
            "permissions" => d.permissions = keys(value),
            "dependencies" => {
                for group in ["bootstrap", "server"] {
                    for (name, dep) in value[group].as_object().into_iter().flatten() {
                        paper.push((name.clone(), dep["required"].as_bool() != Some(false)));
                    }
                }
            }
            _ => {
                if let Some(list) = list_field(&mut d, key) {
                    value.as_array().into_iter().flatten().for_each(|item| add(list, text(item)));
                }
            }
        }
    }
    paper_dependencies(&mut d, paper);
    d
}

/// `groupId`, `artifactId`, `version` from a `pom.properties` file.
pub fn pom_properties(text: &str) -> Option<(String, String, String)> {
    let get = |key: &str| {
        text.lines().find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == key).then(|| v.trim().to_string())
        })
    };
    Some((get("groupId")?, get("artifactId")?, get("version")?))
}

/// Java-agent attributes present in a manifest.
pub fn agent_attributes(manifest: &str) -> Vec<&'static str> {
    ["Premain-Class", "Agent-Class", "Launcher-Agent-Class"]
        .into_iter()
        .filter(|attr| {
            manifest.lines().any(|l| {
                l.split_once(':').is_some_and(|(k, v)| k.trim().eq_ignore_ascii_case(attr) && !v.trim().is_empty())
            })
        })
        .collect()
}

/// Classes a manifest names to start: `Main-Class` and the Java-agent entry classes.
pub fn manifest_classes(manifest: &str) -> Vec<String> {
    // Long values wrap onto continuation lines that start with a space.
    let unfolded = manifest.replace("\r\n ", "").replace("\n ", "");
    unfolded
        .lines()
        .filter_map(|l| {
            let (k, v) = l.split_once(':')?;
            let named = ["Main-Class", "Premain-Class", "Agent-Class", "Launcher-Agent-Class"].iter().any(|a| k.trim().eq_ignore_ascii_case(a));
            (named && !v.trim().is_empty()).then(|| v.trim().to_string())
        })
        .collect()
}

/// log4j-core versions affected by Log4Shell and its follow-ups (fixed in 2.17.1, with
/// patched backports 2.12.4 for Java 7 and 2.3.2 for Java 6).
/// Source: https://logging.apache.org/security.html
pub fn log4j_vulnerable(group: &str, artifact: &str, version: &str) -> bool {
    if group != "org.apache.logging.log4j" || artifact != "log4j-core" {
        return false;
    }
    let nums: Vec<u32> = version
        .split(['.', '-'])
        .map_while(|p| p.parse().ok())
        .collect();
    let at = |i: usize| nums.get(i).copied().unwrap_or(0);
    let (major, minor, patch) = (at(0), at(1), at(2));
    major == 2
        && (minor, patch) < (17, 1)
        && !(minor == 12 && patch >= 4)
        && !(minor == 3 && patch >= 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_yml_subset() {
        let yml = "name: 'Example'\nversion: \"1.2.0\" # release\nmain: com.example.Main\napi-version: '1.20'\nfolia-supported: true\n\
                   commands:\n  example:\n    description: Does things\n    aliases: [ex]\n  other: {}\n\
                   permissions:\n  'example.use':\n    default: true\n  example.admin:\n    children:\n      example.use: true\n\
                   libraries:\n  - com.google.code.gson:gson:2.10.1\n";
        let d = yaml_descriptor(yml);
        assert_eq!(d.name.as_deref(), Some("Example"));
        assert_eq!(d.version.as_deref(), Some("1.2.0"));
        assert_eq!(d.main.as_deref(), Some("com.example.Main"));
        assert!(d.folia_supported);
        assert_eq!(d.commands, ["example", "other"]);
        assert_eq!(d.permissions, ["example.use", "example.admin"]);
        assert_eq!(d.maven_libraries, ["com.google.code.gson:gson:2.10.1"]);
    }

    #[test]
    fn other_descriptors() {
        let v = descriptor("velocity-plugin.json", r#"{"id":"ex","version":"2","main":"a.B"}"#).unwrap();
        assert_eq!((v.name.as_deref(), v.main.as_deref(), v.platform.as_str()), (Some("ex"), Some("a.B"), "velocity"));
        let f = descriptor("META-INF/mods.toml", "[[mods]]\nmodId=\"ex\"\nversion = \"1.0\"\n").unwrap();
        assert_eq!((f.name.as_deref(), f.version.as_deref()), (Some("ex"), Some("1.0")));
        assert!(descriptor("velocity-plugin.json", "{oops").is_none());
    }

    #[test]
    fn authors_and_dependencies() {
        // Lists in flow style, indented, and at the key's own indent, which YAML allows.
        let yml = "name: X\nauthor: Ann\nauthors: [Bo, 'Ann']\nwebsite: https://x.example.com\ndescription: |\n  name: Wrong\n  - Wrong\n\
                   depend:\n- Vault\n- \"LuckPerms\"\nsoftdepend:\n  - PlaceholderAPI\nlibraries:\n- a:b:1\n";
        let d = yaml_descriptor(yml);
        assert_eq!(d.name.as_deref(), Some("X"));
        assert_eq!(d.authors, ["Ann", "Bo"]);
        assert_eq!(d.website.as_deref(), Some("https://x.example.com"));
        assert_eq!(d.dependencies, ["Vault", "LuckPerms"]);
        assert_eq!(d.optional_dependencies, ["PlaceholderAPI"]);
        assert_eq!(d.maven_libraries, ["a:b:1"]);

        let paper = "dependencies:\n  bootstrap:\n    Lib:\n      required: false\n  server:\n    Vault:\n      load: BEFORE\n      required: false\n    Lib:\n      required: true\n    Core: {}\n";
        let p = yaml_descriptor(paper);
        assert_eq!((p.dependencies, p.optional_dependencies), (vec!["Lib".to_string(), "Core".into()], vec!["Vault".to_string()]));

        let b = descriptor("bungee.yml", "name: X\nauthor: Ann\ndepends: [A]\nsoftDepends: [B]\n").unwrap();
        assert_eq!((b.authors, b.dependencies, b.optional_dependencies), (vec!["Ann".to_string()], vec!["A".to_string()], vec!["B".to_string()]));

        let paper_json = "{\n  \"name\": \"x\",\n  \"main\": \"a.B\",\n  \"authors\": [\"Ann\"],\n  \"website\": \"https://x.example.com\",\n  \"dependencies\": {\"server\": {\"Vault\": {\"required\": false}, \"Core\": {}}}\n}\n";
        let j = descriptor("paper-plugin.yml", paper_json).unwrap();
        assert_eq!((j.name.as_deref(), j.main.as_deref(), j.website.as_deref()), (Some("x"), Some("a.B"), Some("https://x.example.com")));
        assert_eq!((j.authors, j.dependencies, j.optional_dependencies), (vec!["Ann".to_string()], vec!["Core".to_string()], vec!["Vault".to_string()]));

        let json = r#"{"id":"x","authors":["Ann"],"url":"https://x.example.com","dependencies":[{"id":"luckperms"},{"id":"geyser","optional":true}]}"#;
        let v = descriptor("velocity-plugin.json", json).unwrap();
        assert_eq!((v.authors, v.website.as_deref()), (vec!["Ann".to_string()], Some("https://x.example.com")));
        assert_eq!((v.dependencies, v.optional_dependencies), (vec!["luckperms".to_string()], vec!["geyser".to_string()]));
    }

    #[test]
    fn log4j_versions() {
        let v = |s| log4j_vulnerable("org.apache.logging.log4j", "log4j-core", s);
        assert!(v("2.14.1") && v("2.0-beta9") && v("2.17.0") && v("2.12.1") && v("2.3"));
        assert!(!v("2.17.1") && !v("2.20.0") && !v("2.12.4") && !v("2.3.2") && !v("1.2.17"));
        assert!(!log4j_vulnerable("org.apache.logging.log4j", "log4j-api", "2.14.1"));
    }

    #[test]
    fn manifest_agents() {
        assert_eq!(agent_attributes("Manifest-Version: 1.0\r\nPremain-Class: a.B\r\n"), ["Premain-Class"]);
        assert!(agent_attributes("Main-Class: a.B\n").is_empty());
        assert_eq!(manifest_classes("Main-Class: a.b.VeryLongName\r\n Continued\r\nAgent-Class: c.D\r\nX: y\r\n"), ["a.b.VeryLongNameContinued", "c.D"]);
    }
}
