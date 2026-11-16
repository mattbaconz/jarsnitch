//! `jarsnitch check`: compares installed jars with the official releases on Modrinth and
//! Hangar, and looks for updates there. The only command that uses the network.

use std::cell::Cell;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use jarsnitch_core::{Diff, Limits, PluginInfo, Report};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha512};

pub const MODRINTH: &str = "https://api.modrinth.com/v2";
pub const HANGAR: &str = "https://hangar.papermc.io/api/v1";
/// Largest response read, official jars included.
const MAX_BODY: u64 = 256 << 20;
/// Official files downloaded per jar while looking for the one it should match.
const MAX_CANDIDATES: usize = 3;

pub trait Http {
    /// The body of a GET, or `None` on 404.
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>, String>;
    /// The body of a POST with a JSON body.
    fn post(&self, url: &str, json: &str) -> Result<Vec<u8>, String>;
}

/// HTTPS through ureq, paced to stay under Modrinth's 300 and Hangar's 240 requests a minute.
pub struct Net {
    agent: ureq::Agent,
    last: Cell<Option<Instant>>,
}

impl Net {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent(concat!(
                "mattbaconz/jarsnitch/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/mattbaconz/jarsnitch)"
            ))
            .timeout_global(Some(Duration::from_secs(120)))
            .http_status_as_error(false)
            // No proxy from the environment, and never plain HTTP, even after a redirect.
            .proxy(None)
            .https_only(true)
            .build()
            .into();
        Net {
            agent,
            last: Cell::new(None),
        }
    }

    fn pace(&self, url: &str) {
        // Downloads come from CDNs, which these limits don't cover.
        if !(url.starts_with(MODRINTH) || url.starts_with(HANGAR)) {
            return;
        }
        let gap = Duration::from_millis(if url.starts_with(HANGAR) { 260 } else { 210 });
        if let Some(t) = self.last.get() {
            std::thread::sleep(gap.saturating_sub(t.elapsed()));
        }
        self.last.set(Some(Instant::now()));
    }

    fn send(&self, url: &str, body: Option<&str>) -> Result<Option<Vec<u8>>, String> {
        let short = url.split('?').next().unwrap_or(url);
        for attempt in 0..2 {
            self.pace(url);
            let result = match body {
                Some(b) => self
                    .agent
                    .post(url)
                    .header("Content-Type", "application/json")
                    .send(b),
                None => self.agent.get(url).call(),
            };
            let mut r = result.map_err(|e| format!("{short}: {e}"))?;
            match r.status().as_u16() {
                200 => {
                    return r
                        .body_mut()
                        .with_config()
                        .limit(MAX_BODY)
                        .read_to_vec()
                        .map(Some)
                        .map_err(|e| format!("{short}: {e}"));
                }
                404 => return Ok(None),
                429 if attempt == 0 => {
                    let wait = r
                        .headers()
                        .get("x-ratelimit-reset")
                        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
                        .unwrap_or(10);
                    std::thread::sleep(Duration::from_secs(wait.clamp(1, 60)));
                }
                s => return Err(format!("{short}: HTTP {s}")),
            }
        }
        Err(format!("{short}: rate limited"))
    }
}

impl Http for Net {
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>, String> {
        self.send(url, None)
    }
    fn post(&self, url: &str, json: &str) -> Result<Vec<u8>, String> {
        self.send(url, Some(json))?
            .ok_or_else(|| format!("{url}: HTTP 404"))
    }
}

/// One installed jar, checked.
#[derive(Debug, Serialize)]
pub struct Checked {
    pub file: String,
    pub sha256: String,
    pub plugin: Option<String>,
    pub version: Option<String>,
    /// `official`: these exact bytes are a file on Modrinth or Hangar.
    /// `same-code`: different bytes from the official file of this version, same code.
    /// `differs`: the official file of this version has different code; see `official.diff`.
    /// `not-found`: neither site has these bytes or a file of this version.
    /// `unreadable`: not a jar.
    pub status: &'static str,
    pub official: Option<Source>,
    /// With `not-found`: a file of this name and version in the project named like the
    /// plugin that isn't the same plugin (its main class and most classes differ), with
    /// the comparison. Otherwise `null`.
    pub unlike: Option<Source>,
    pub update: Option<Source>,
    /// Requests that failed; the result may be incomplete.
    pub errors: Vec<String>,
}

/// A release on Modrinth or Hangar.
#[derive(Debug, Serialize)]
pub struct Source {
    /// `modrinth` or `hangar`.
    pub site: &'static str,
    pub project: String,
    /// The project's page, or the version's page for an update.
    pub url: String,
    pub version: Option<String>,
    pub published: Option<String>,
    pub file: Option<String>,
    pub download: Option<String>,
    /// For `official`: the official file compared with this jar (old is the official
    /// file). For `update`: this jar compared with the update (new is the update).
    pub diff: Option<Diff>,
}

impl Checked {
    /// This jar's code differs from the official file, it adds a High finding to it, or
    /// the update adds a High finding.
    pub fn alarming(&self) -> bool {
        let adds_high = |s: &Option<Source>| {
            s.as_ref()
                .and_then(|s| s.diff.as_ref())
                .is_some_and(Diff::adds_high)
        };
        self.status == "differs" || adds_high(&self.official) || adds_high(&self.update)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct MrVersion {
    id: String,
    project_id: String,
    version_number: String,
    #[serde(default)]
    version_type: String,
    #[serde(default)]
    date_published: String,
    #[serde(default)]
    game_versions: Vec<String>,
    files: Vec<MrFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct MrFile {
    hashes: MrHashes,
    url: String,
    filename: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct MrHashes {
    sha512: String,
}

#[derive(Debug, Clone, Deserialize)]
struct MrProject {
    id: String,
    slug: String,
    title: String,
}

/// Where a jar's release lives, for the update lookup.
enum Home {
    Modrinth {
        project: MrProject,
        file: MrFile,
    },
    /// `sha` is the SHA-256 of the official file the jar was matched with.
    Hangar {
        slug: String,
        page: String,
        project: String,
        version: Option<String>,
        sha: Option<String>,
    },
}

struct Jar {
    bytes: Vec<u8>,
    report: Report,
    sha512: String,
    loaders: Vec<&'static str>,
    out: Checked,
    home: Option<Home>,
}

impl Jar {
    fn plugin(&self) -> Option<&PluginInfo> {
        main_plugin(&self.report)
    }
    fn server_plugin(&self) -> bool {
        self.report.plugins.iter().any(|p| {
            matches!(
                p.platform.as_str(),
                "bukkit" | "paper" | "bungee" | "velocity"
            )
        })
    }
}

/// The descriptor that names the plugin: `plugin.yml` before the others.
fn main_plugin(r: &Report) -> Option<&PluginInfo> {
    const ORDER: [&str; 8] = [
        "bukkit", "paper", "velocity", "bungee", "fabric", "quilt", "neoforge", "forge",
    ];
    r.plugins.iter().min_by_key(|p| {
        ORDER
            .iter()
            .position(|o| *o == p.platform)
            .unwrap_or(ORDER.len())
    })
}

/// Modrinth loaders a jar with these descriptors runs on.
fn loaders(r: &Report) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for p in &r.plugins {
        let l: &[&'static str] = match p.platform.as_str() {
            "bukkit" => &["bukkit", "spigot", "paper", "purpur", "folia"],
            "paper" => &["paper", "purpur", "folia"],
            "bungee" => &["bungeecord", "waterfall"],
            "velocity" => &["velocity"],
            "fabric" => &["fabric"],
            "quilt" => &["quilt"],
            "forge" => &["forge"],
            "neoforge" => &["neoforge"],
            _ => &[],
        };
        for x in l {
            if !out.contains(x) {
                out.push(x);
            }
        }
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Percent-encodes everything but unreserved characters, for query strings.
fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn parse<T: for<'de> Deserialize<'de>>(body: &[u8], what: &str) -> Result<T, String> {
    serde_json::from_slice(body).map_err(|e| format!("{what}: unexpected response ({e})"))
}

/// `1.4.28` matches `1.4.28`, `v1.4.28` and `1.4.28-bukkit`, not `1.4.2`, `1.4.28.1`,
/// `1.4.28-SNAPSHOT`, `1.4.28-beta.1` or `1.4.28-b45`: a development or pre-release build
/// isn't the release, and comparing them would only report differences.
fn same_version(claimed: &str, listed: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
    let (a, b) = (norm(claimed), norm(listed));
    let extends = |long: &str, short: &str| {
        long.strip_prefix(short)
            .is_some_and(|rest| rest.starts_with(['-', '+', '_', ' ', '(']) && !prerelease(rest))
    };
    !a.is_empty() && (a == b || extends(&b, &a) || extends(&a, &b))
}

/// A version tail with a pre-release word in it: `-SNAPSHOT`, `-rc1`, `+build.5`, `-M2`,
/// `-b45`. Whole words only, so `-premium` and `-sources` aren't.
fn prerelease(tail: &str) -> bool {
    const PRE: [&str; 14] = [
        "snapshot",
        "dev",
        "alpha",
        "beta",
        "rc",
        "pre",
        "preview",
        "nightly",
        "build",
        "test",
        "experimental",
        "unstable",
        "canary",
        "ea",
    ];
    tail.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|token| {
            let word = token.trim_end_matches(|c: char| c.is_ascii_digit());
            PRE.contains(&word) || (["m", "b"].contains(&word) && word.len() < token.len())
        })
}

/// `published` is later than `than`. Both are ISO 8601 times from the same site; an
/// unknown time doesn't block an update.
fn newer(published: Option<&str>, than: Option<&str>) -> bool {
    // "2026-11-01T10:00:00.123456Z": the seconds, then the fraction to 9 digits, so dates with
    // and without one compare. An unknown date counts as newer.
    let key = |s: &str| {
        let secs = s.get(..19).unwrap_or(s);
        let frac: String = s
            .get(19..)
            .unwrap_or("")
            .strip_prefix('.')
            .unwrap_or("")
            .chars()
            .take_while(char::is_ascii_digit)
            .take(9)
            .collect();
        (secs.to_string(), format!("{frac:0<9}"))
    };
    match (published, than) {
        (Some(p), Some(t)) if !p.is_empty() && !t.is_empty() => key(p) > key(t),
        _ => true,
    }
}

/// A file name without its version: `Chunky-Bukkit-1.4.28.jar` and `Chunky-Bukkit-1.4.36.jar`
/// share one, `Chunky-Paper-1.4.36.jar` doesn't.
fn stem(file: &str) -> String {
    file.to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_ascii_digit() && !".-_+".contains(*c))
        .collect()
}

/// The same version name, ignoring case and a leading `v`.
fn exact_version(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.trim().trim_start_matches(['v', 'V']).to_ascii_lowercase();
    !norm(a).is_empty() && norm(a) == norm(b)
}

/// A version without its suffix: `5.29` of `5.29-paper`, `1.0.0` of `v1.0.0+build.5`.
fn base_version(v: &str) -> String {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .to_ascii_lowercase()
        .split(['-', '+', '_', ' ', '('])
        .next()
        .unwrap_or_default()
        .to_string()
}

/// A file name, lowercase, without its release's version: `Fixture-1.0.0-1.8.jar` of 1.0.0 is
/// `fixture--1.8.jar`, so builds for other Minecraft versions stay apart.
fn bare(file: &str, version: &str) -> String {
    let (file, version) = (
        file.to_ascii_lowercase(),
        version.trim_start_matches(['v', 'V']).to_ascii_lowercase(),
    );
    if version.is_empty() {
        file
    } else {
        file.replacen(&version, "", 1)
    }
}

/// Renaming a project cannot establish that a different platform or Minecraft build is
/// compatible. Keep explicit platform words (even inside a release version) and any
/// numeric markers left after removing that release version.
fn rename_markers(file: &str, version: &str) -> (BTreeSet<String>, Vec<String>, Vec<String>) {
    let platforms = file
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|s| {
            matches!(
                *s,
                "bukkit"
                    | "spigot"
                    | "paper"
                    | "purpur"
                    | "folia"
                    | "fabric"
                    | "quilt"
                    | "forge"
                    | "neoforge"
                    | "velocity"
                    | "bungee"
                    | "bungeecord"
                    | "waterfall"
            )
        })
        .map(str::to_string)
        .collect();
    let numbers = bare(file, version)
        .split(|c: char| !c.is_ascii_digit() && c != '.')
        .map(|s| s.trim_matches('.'))
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    (platforms, numbers, minecraft_markers(file))
}

/// An explicit `mc1.20.1` or `minecraft-1.20.1` label remains a build constraint even
/// when the publisher includes it in the release version itself.
fn minecraft_markers(file: &str) -> Vec<String> {
    let lower = file.to_ascii_lowercase();
    let parts: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .collect();
    parts
        .iter()
        .enumerate()
        .filter_map(|(i, s)| {
            let tail = s
                .strip_prefix("minecraft")
                .or_else(|| s.strip_prefix("mc"))?;
            let tail = if tail.is_empty() {
                *parts.get(i + 1)?
            } else {
                tail
            };
            let number = tail
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .next()?
                .trim_matches('.');
            (!number.is_empty()).then(|| number.to_string())
        })
        .collect()
}

/// Checks `jars` (file name and bytes) against Modrinth and Hangar.
pub fn run(
    http: &dyn Http,
    jars: Vec<(String, Vec<u8>)>,
    mc: Option<&str>,
    updates: bool,
    limits: &Limits,
) -> Vec<Checked> {
    let mut jars: Vec<Jar> = jars
        .into_iter()
        .map(|(name, bytes)| {
            let report = jarsnitch_core::scan(&name, &bytes, limits);
            let plugin = main_plugin(&report);
            let out = Checked {
                file: name,
                sha256: report.file.sha256.clone(),
                plugin: plugin.and_then(|p| p.name.clone()),
                version: plugin.and_then(|p| p.version.clone()),
                status: if report.is_unreadable() {
                    "unreadable"
                } else {
                    "not-found"
                },
                official: None,
                unlike: None,
                update: None,
                errors: Vec::new(),
            };
            Jar {
                sha512: hex(&Sha512::digest(&bytes)),
                loaders: loaders(&report),
                bytes,
                report,
                out,
                home: None,
            }
        })
        .collect();
    let mut live: Vec<&mut Jar> = jars
        .iter_mut()
        .filter(|j| j.out.status != "unreadable")
        .collect();

    by_hash_modrinth(http, &mut live);
    for j in live
        .iter_mut()
        .filter(|j| j.home.is_none() && j.server_plugin())
    {
        if let Err(e) = by_hash_hangar(http, j) {
            j.out.errors.push(e);
        }
    }
    for j in live.iter_mut().filter(|j| j.home.is_none()) {
        if let Err(e) = locate(http, j, limits) {
            j.out.errors.push(e);
        }
    }
    if updates {
        updates_modrinth(http, &mut live, mc, limits);
        for j in live.iter_mut() {
            if let Err(e) = update_hangar(http, j, mc, limits) {
                j.out.errors.push(e);
            }
        }
    }
    jars.into_iter().map(|j| j.out).collect()
}

fn modrinth_source(project: &MrProject, version: &MrVersion, file: &MrFile) -> Source {
    Source {
        site: "modrinth",
        project: project.title.clone(),
        url: format!("https://modrinth.com/project/{}", project.slug),
        version: Some(version.version_number.clone()),
        published: Some(version.date_published.clone()).filter(|d| !d.is_empty()),
        file: Some(file.filename.clone()),
        download: Some(file.url.clone()),
        diff: None,
    }
}

fn projects(http: &dyn Http, ids: &[&str]) -> Result<Vec<MrProject>, String> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("{MODRINTH}/projects?ids={}", enc(&json!(ids).to_string()));
    Ok(match http.get(&url)? {
        Some(b) => parse(&b, "Modrinth projects")?,
        None => Vec::new(),
    })
}

/// Exact bytes on Modrinth: one request for every jar.
fn by_hash_modrinth(http: &dyn Http, jars: &mut [&mut Jar]) {
    if jars.is_empty() {
        return;
    }
    let hashes: Vec<&str> = jars.iter().map(|j| j.sha512.as_str()).collect();
    let body = json!({ "hashes": hashes, "algorithm": "sha512" }).to_string();
    let found = http
        .post(&format!("{MODRINTH}/version_files"), &body)
        .and_then(|b| {
            parse::<std::collections::BTreeMap<String, MrVersion>>(&b, "Modrinth version_files")
        });
    let found = match found {
        Ok(f) => f,
        Err(e) => return jars.iter_mut().for_each(|j| j.out.errors.push(e.clone())),
    };
    let ids: Vec<&str> = found.values().map(|v| v.project_id.as_str()).collect();
    let projects = match projects(http, &ids) {
        Ok(p) => p,
        Err(e) => return jars.iter_mut().for_each(|j| j.out.errors.push(e.clone())),
    };
    for j in jars.iter_mut() {
        let Some(version) = found.get(&j.sha512) else {
            continue;
        };
        let (Some(project), Some(file)) = (
            projects.iter().find(|p| p.id == version.project_id),
            version.files.iter().find(|f| f.hashes.sha512 == j.sha512),
        ) else {
            continue;
        };
        j.out.status = "official";
        j.out.official = Some(modrinth_source(project, version, file));
        j.home = Some(Home::Modrinth {
            project: project.clone(),
            file: file.clone(),
        });
    }
}

/// Hangar's platform names for a jar: Paper first, then the proxies.
fn hangar_platforms(j: &Jar) -> Vec<&'static str> {
    let has = |names: &[&str]| {
        j.report
            .plugins
            .iter()
            .any(|p| names.contains(&p.platform.as_str()))
    };
    [
        ("PAPER", &["bukkit", "paper"][..]),
        ("VELOCITY", &["velocity"][..]),
        ("WATERFALL", &["bungee"][..]),
    ]
    .into_iter()
    .filter(|(_, names)| has(names))
    .map(|(h, _)| h)
    .collect()
}

fn hangar_project(v: &Value) -> Option<(String, String, String)> {
    let owner = v["namespace"]["owner"].as_str()?;
    let slug = v["namespace"]["slug"].as_str()?;
    let name = v["name"].as_str().unwrap_or(slug);
    Some((
        slug.to_string(),
        format!("https://hangar.papermc.io/{owner}/{slug}"),
        name.to_string(),
    ))
}

/// A Hangar version's downloadable files for these platforms: url, name, sha256.
fn hangar_files(v: &Value, platforms: &[&str]) -> Vec<(String, String, String)> {
    platforms
        .iter()
        .filter_map(|p| {
            let d = &v["downloads"][*p];
            let info = &d["fileInfo"];
            Some((
                d["downloadUrl"].as_str()?.to_string(),
                info["name"].as_str()?.to_string(),
                info["sha256Hash"].as_str()?.to_ascii_lowercase(),
            ))
        })
        .collect()
}

/// Exact bytes on Hangar, which looks up SHA-256 and answers with the project only.
fn by_hash_hangar(http: &dyn Http, j: &mut Jar) -> Result<(), String> {
    let Some(body) = http.get(&format!("{HANGAR}/versions/hash/{}", j.out.sha256))? else {
        return Ok(());
    };
    let v: Value = parse(&body, "Hangar versions/hash")?;
    let Some((slug, page, project)) = hangar_project(&v) else {
        return Ok(());
    };
    j.out.status = "official";
    j.out.official = Some(Source {
        site: "hangar",
        project: project.clone(),
        url: page.clone(),
        version: None,
        published: None,
        file: None,
        download: None,
        diff: None,
    });
    j.home = Some(Home::Hangar {
        slug: slug.clone(),
        page,
        project,
        version: None,
        sha: Some(j.out.sha256.clone()),
    });
    // Pin the version when it's the one the jar claims. The match stands without it.
    let Some(claimed) = j.out.version.clone() else {
        return Ok(());
    };
    let Some(b) = http.get(&format!(
        "{HANGAR}/projects/{}/versions/{}",
        enc(&slug),
        enc(&claimed)
    ))?
    else {
        return Ok(());
    };
    let v: Value = parse(&b, "Hangar version")?;
    if let Some((_, name, _)) = hangar_files(&v, &["PAPER", "WATERFALL", "VELOCITY"])
        .into_iter()
        .find(|(_, _, sha)| *sha == j.out.sha256)
        && let Some(o) = j.out.official.as_mut()
    {
        o.version = v["name"].as_str().map(str::to_string);
        o.published = v["createdAt"].as_str().map(str::to_string);
        o.file = Some(name);
        if let Some(Home::Hangar { version, .. }) = j.home.as_mut() {
            *version = o.version.clone();
        }
    }
    Ok(())
}

/// Downloads an official file, checks its hash, and compares it with the jar; `true` when
/// it's the same plugin: the same name, and the same main class or at least half of the
/// official classes (so a changed `main:` can't hide a modified copy).
fn compare(
    http: &dyn Http,
    j: &Jar,
    url: &str,
    name: &str,
    sha: (&str, &str),
    limits: &Limits,
) -> Result<(Diff, bool), String> {
    let Some(bytes) = http.get(url)? else {
        return Err(format!("{url}: HTTP 404"));
    };
    let actual = match sha.0 {
        "sha512" => hex(&Sha512::digest(&bytes)),
        _ => hex(&sha2::Sha256::digest(&bytes)),
    };
    if actual != sha.1 {
        return Err(format!(
            "{url}: the download doesn't match the {} the site lists",
            sha.0
        ));
    }
    let d = jarsnitch_core::diff(name, &bytes, &j.out.file, &j.bytes, limits);
    let (theirs, ours) = (
        d.old
            .plugins
            .iter()
            .find(|p| Some(&p.platform) == j.plugin().map(|q| &q.platform)),
        j.plugin(),
    );
    let same = |a: Option<&String>, b: Option<&String>| match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    let c = &d.code;
    let shared = c.unchanged_classes as usize + c.changed_classes.len();
    let mostly_same = 2 * shared >= shared + c.removed_classes.len() && shared > 0;
    let same_plugin = theirs.zip(ours).is_some_and(|(a, b)| {
        same(a.name.as_ref(), b.name.as_ref()) && (a.main == b.main || mostly_same)
    });
    Ok((d, same_plugin))
}

/// The body, or `None` after noting the error.
fn keep(r: Result<Option<Vec<u8>>, String>, errors: &mut Vec<String>) -> Option<Vec<u8>> {
    r.map_err(|e| errors.push(e)).ok().flatten()
}

/// How far a comparison is from identical, for picking the closest official file.
fn distance(d: &Diff) -> usize {
    let c = &d.code;
    c.changed_classes.len()
        + c.added_classes.len()
        + c.removed_classes.len()
        + c.changed_files.len()
        + c.added_files.len()
        + c.removed_files.len()
}

/// Bytes not found: look for the official file of the version the jar claims, and compare.
fn locate(http: &dyn Http, j: &mut Jar, limits: &Limits) -> Result<(), String> {
    let (Some(name), Some(claimed)) = (j.out.plugin.clone(), j.out.version.clone()) else {
        return Ok(());
    };
    if j.loaders.is_empty() {
        return Ok(());
    }
    let mut best: Option<(Diff, Source, Home)> = None;
    let consider = |d: Diff, s: Source, h: Home, best: &mut Option<(Diff, Source, Home)>| {
        if best
            .as_ref()
            .is_none_or(|(b, ..)| distance(&d) < distance(b))
        {
            *best = Some((d, s, h));
        }
    };

    // A failed request or a bad download skips that candidate, not the rest, and is
    // reported: the result may then come from a less likely file.
    let mut errors = Vec::new();
    // A file of this version in the plugin's own project that isn't the same plugin.
    let mut unlike: Option<Source> = None;

    // Modrinth: the project named like the plugin, then search hits.
    let kind = if j.server_plugin() { "plugin" } else { "mod" };
    let guess = name.to_ascii_lowercase().replace(' ', "-");
    let mut candidates = projects(http, &[guess.as_str()]).unwrap_or_else(|e| {
        errors.push(e);
        Vec::new()
    });
    let facets = json!([[format!("project_type:{kind}")]]).to_string();
    let hits = keep(
        http.get(&format!(
            "{MODRINTH}/search?query={}&facets={}&limit=3",
            enc(&name),
            enc(&facets)
        )),
        &mut errors,
    );
    if let Some(hits) = hits
        .map(|b| parse::<Value>(&b, "Modrinth search"))
        .transpose()
        .unwrap_or_else(|e| {
            errors.push(e);
            None
        })
    {
        let ids: Vec<String> = hits["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|h| h["project_id"].as_str().map(str::to_string))
            .filter(|id| !candidates.iter().any(|c| c.id == *id))
            .collect();
        match projects(http, &ids.iter().map(String::as_str).collect::<Vec<_>>()) {
            Ok(more) => candidates.extend(more),
            Err(e) => errors.push(e),
        }
    }
    let mut downloads = 0;
    let loaders = enc(&json!(j.loaders).to_string());
    'projects: for project in candidates.iter().take(3) {
        let got = http.get(&format!(
            "{MODRINTH}/project/{}/version?loaders={loaders}&include_changelog=false",
            project.id
        ));
        let versions: Vec<MrVersion> =
            match got.and_then(|b| b.map(|b| parse(&b, "Modrinth versions")).transpose()) {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
        let mut matching: Vec<&MrVersion> = versions
            .iter()
            .filter(|v| same_version(&claimed, &v.version_number))
            .collect();
        matching.sort_by_key(|v| !v.version_number.eq_ignore_ascii_case(&claimed));
        for version in matching {
            let mut files: Vec<&MrFile> = version
                .files
                .iter()
                .filter(|f| f.filename.to_ascii_lowercase().ends_with(".jar"))
                .collect();
            files.sort_by_key(|f| (f.filename != j.out.file, !f.primary));
            for file in files {
                if downloads >= MAX_CANDIDATES {
                    break 'projects;
                }
                downloads += 1;
                match compare(
                    http,
                    j,
                    &file.url,
                    &file.filename,
                    ("sha512", &file.hashes.sha512),
                    limits,
                ) {
                    Ok((d, true)) => {
                        let home = Home::Modrinth {
                            project: project.clone(),
                            file: file.clone(),
                        };
                        consider(d, modrinth_source(project, version, file), home, &mut best);
                        if best.as_ref().is_some_and(|(d, ..)| distance(d) == 0) {
                            break 'projects;
                        }
                    }
                    Ok((d, false)) if unlike.is_none() && project.slug == guess => {
                        unlike = Some(Source {
                            diff: Some(d),
                            ..modrinth_source(project, version, file)
                        });
                    }
                    Ok(_) => {}
                    Err(e) => errors.push(e),
                }
            }
        }
        if best.is_some() {
            break;
        }
    }

    // Hangar, by the plugin's name as the slug.
    if best.is_none() && j.server_plugin() {
        let project = keep(
            http.get(&format!(
                "{HANGAR}/projects/{}",
                enc(&name.replace(' ', "-"))
            )),
            &mut errors,
        )
        .and_then(|b| match parse(&b, "Hangar project") {
            Ok(v) => hangar_project(&v),
            Err(e) => {
                errors.push(e);
                None
            }
        });
        if let Some((slug, page, _)) = project
            && let Some(b) = keep(
                http.get(&format!(
                    "{HANGAR}/projects/{}/versions/{}",
                    enc(&slug),
                    enc(&claimed)
                )),
                &mut errors,
            )
        {
            match parse::<Value>(&b, "Hangar version") {
                Ok(v) => {
                    let version = v["name"].as_str().map(str::to_string);
                    let published = v["createdAt"].as_str().map(str::to_string);
                    for (url, file, sha) in hangar_files(&v, &hangar_platforms(j))
                        .into_iter()
                        .take(MAX_CANDIDATES)
                    {
                        let result = compare(http, j, &url, &file, ("sha256", &sha), limits);
                        let source = Source {
                            site: "hangar",
                            project: name.clone(),
                            url: page.clone(),
                            version: version.clone(),
                            published: published.clone(),
                            file: Some(file),
                            download: Some(url),
                            diff: None,
                        };
                        match result {
                            Ok((d, true)) => {
                                let home = Home::Hangar {
                                    slug: slug.clone(),
                                    page: page.clone(),
                                    project: name.clone(),
                                    version: version.clone(),
                                    sha: Some(sha.clone()),
                                };
                                consider(d, source, home, &mut best);
                            }
                            Ok((d, false)) if unlike.is_none() => {
                                unlike = Some(Source {
                                    diff: Some(d),
                                    ..source
                                })
                            }
                            Ok(_) => {}
                            Err(e) => errors.push(e),
                        }
                    }
                }
                Err(e) => errors.push(e),
            }
        }
    }

    match best {
        Some((d, mut source, home)) => {
            j.out.status = if d.same_code() {
                "same-code"
            } else {
                "differs"
            };
            source.diff = Some(d);
            j.out.official = Some(source);
            j.home = Some(home);
        }
        None => j.out.unlike = unlike,
    }
    j.out.errors.extend(errors);
    Ok(())
}

fn updates_modrinth(http: &dyn Http, jars: &mut [&mut Jar], mc: Option<&str>, limits: &Limits) {
    let game_versions: Vec<&str> = mc.into_iter().collect();
    let hashes: Vec<Value> = jars
        .iter()
        .filter_map(|j| match &j.home {
            Some(Home::Modrinth { file, .. }) => {
                Some(json!({ "hash": file.hashes.sha512, "loaders": j.loaders, "game_versions": game_versions, "version_types": ["release"] }))
            }
            _ => None,
        })
        .collect();
    if hashes.is_empty() {
        return;
    }
    let body = json!({ "algorithm": "sha512", "hashes": hashes }).to_string();
    let found = http
        .post(
            &format!("{MODRINTH}/version_files/update_individual"),
            &body,
        )
        .and_then(|b| {
            parse::<std::collections::BTreeMap<String, MrVersion>>(&b, "Modrinth update_individual")
        });
    let found = match found {
        Ok(f) => f,
        Err(e) => return jars.iter_mut().for_each(|j| j.out.errors.push(e.clone())),
    };
    for j in jars.iter_mut() {
        let Some(Home::Modrinth { project, file, .. }) = &j.home else {
            continue;
        };
        let Some(newest) = found.get(&file.hashes.sha512) else {
            continue;
        };
        let (project, current) = (project.clone(), file.clone());
        if let Err(e) = update_from(http, j, &project, &current, newest, mc, limits) {
            j.out.errors.push(e);
        }
    }
}

/// `newest` is the latest release of the project; the update is the latest one with a
/// file of the same build (Bukkit or Paper, Minecraft version) as the installed one.
fn update_from(
    http: &dyn Http,
    j: &mut Jar,
    project: &MrProject,
    current: &MrFile,
    newest: &MrVersion,
    mc: Option<&str>,
    limits: &Limits,
) -> Result<(), String> {
    let has_current = |v: &MrVersion| {
        v.files
            .iter()
            .any(|f| f.hashes.sha512 == current.hashes.sha512)
    };
    // An update is published after the installed release: a beta install, or --mc for an
    // older Minecraft version, can make the latest matching release an older one.
    let installed_at = j.out.official.as_ref().and_then(|o| o.published.clone());
    let later = |v: &MrVersion| newer(Some(&v.date_published), installed_at.as_deref());
    if has_current(newest) || !later(newest) {
        return Ok(());
    }
    let installed = j
        .out
        .official
        .as_ref()
        .and_then(|o| o.version.clone())
        .unwrap_or_default();
    // A release of the installed version itself (the same file uploaded again, or another
    // build of it) isn't an update; a later build with a suffix, like 1.0.0-1, can be.
    let own = |v: &MrVersion| exact_version(&installed, &v.version_number);
    // Releases of the installed version, suffixes included: 5.29-spigot is a twin of 5.29-paper.
    let twin = |v: &MrVersion| {
        !installed.is_empty() && base_version(&v.version_number) == base_version(&installed)
    };
    let flavor = stem(&current.filename);
    let mine = bare(&current.filename, &installed);
    let minecraft = minecraft_markers(&current.filename);
    let literal = |file: &str, version: &str| bare(file, version) != file.to_ascii_lowercase();
    let mine_literal = literal(&current.filename, &installed);
    let jars = |v: &MrVersion| -> Vec<MrFile> {
        v.files
            .iter()
            .filter(|f| f.filename.to_ascii_lowercase().ends_with(".jar"))
            .cloned()
            .collect()
    };
    // The installed build's file: the same name stem, and the same name once each release's
    // version is taken out, so bluemap-5.29-spigot.jar of 5.29-spigot isn't the build of
    // bluemap-5.29-paper.jar of 5.29-paper and Fixture-1.1.0-1.21.jar isn't the build of
    // Fixture-1.0.0-1.8.jar. When a name doesn't hold its release's version, the only file
    // with the same stem. Otherwise none, rather than the wrong build.
    let pick = |v: &MrVersion| {
        let js = jars(v);
        let alike: Vec<&MrFile> = js
            .iter()
            .filter(|f| stem(&f.filename) == flavor && minecraft_markers(&f.filename) == minecraft)
            .collect();
        if let Some(f) = alike
            .iter()
            .find(|f| bare(&f.filename, &v.version_number) == mine)
        {
            return Some((*f).clone());
        }
        match alike
            .iter()
            .filter(|f| !(mine_literal && literal(&f.filename, &v.version_number)))
            .collect::<Vec<_>>()
            .as_slice()
        {
            [only] => Some((**only).clone()),
            _ => None,
        }
    };
    let mut chosen = if own(newest) {
        None
    } else {
        pick(newest).map(|f| (newest.clone(), f))
    };
    // The file names, without versions, of the releases from the installed one back and of
    // the installed version's twins, to tell a project that renamed its files from one that
    // has always had another flavor.
    let mut older: Option<BTreeSet<String>> = None;
    if chosen.is_none() {
        // The newest release has no file of this build: walk the list, newest first. The list
        // isn't filtered by --mc, so older releases for other Minecraft versions still count
        // for the rename rule; a candidate has to list it.
        let url = format!(
            "{MODRINTH}/project/{}/version?loaders={}&include_changelog=false",
            project.id,
            enc(&json!(j.loaders).to_string())
        );
        if let Some(b) = http.get(&url)? {
            let versions: Vec<MrVersion> = parse(&b, "Modrinth versions")?;
            let fits = |v: &MrVersion| mc.is_none_or(|m| v.game_versions.iter().any(|g| g == m));
            let mut stems = BTreeSet::new();
            let mut back = false;
            for v in versions.iter().filter(|v| v.version_type == "release") {
                // Back at the installed release: nothing newer of this build.
                back |= has_current(v) || !later(v);
                if back || twin(v) {
                    stems.extend(jars(v).iter().map(|f| stem(&f.filename)));
                }
                if !back
                    && !own(v)
                    && fits(v)
                    && let Some(f) = pick(v)
                {
                    chosen = Some((v.clone(), f));
                    break;
                }
            }
            older = Some(stems);
        }
    }
    // A project that renamed its files: the newest release's only jar, unless it's
    // another flavor of the installed version or a flavor older releases have too.
    if chosen.is_none()
        && !twin(newest)
        && let [only] = jars(newest).as_slice()
        && older
            .as_ref()
            .is_none_or(|s| !s.contains(&stem(&only.filename)))
        && rename_markers(&current.filename, &installed)
            == rename_markers(&only.filename, &newest.version_number)
    {
        chosen = Some((newest.clone(), only.clone()));
    }
    let Some((version, file)) = chosen else {
        return Ok(());
    };
    let mut source = modrinth_source(project, &version, &file);
    source.url = format!(
        "https://modrinth.com/project/{}/version/{}",
        project.slug, version.id
    );
    source.diff = download_diff(
        http,
        j,
        &file.url,
        &file.filename,
        ("sha512", &file.hashes.sha512),
        limits,
    )?;
    j.out.update = Some(source);
    Ok(())
}

/// This jar compared with an update.
fn download_diff(
    http: &dyn Http,
    j: &mut Jar,
    url: &str,
    name: &str,
    sha: (&str, &str),
    limits: &Limits,
) -> Result<Option<Diff>, String> {
    let Some(bytes) = http.get(url)? else {
        return Err(format!("{url}: HTTP 404"));
    };
    let actual = if sha.0 == "sha512" {
        hex(&Sha512::digest(&bytes))
    } else {
        hex(&sha2::Sha256::digest(&bytes))
    };
    if actual != sha.1 {
        return Err(format!(
            "{url}: the download doesn't match the {} the site lists",
            sha.0
        ));
    }
    let d = jarsnitch_core::diff(&j.out.file, &j.bytes, name, &bytes, limits);
    if d.is_unreadable() {
        return Err(format!(
            "{url}: the update comparison requires a readable jar"
        ));
    }
    if d.old.partial || d.new.partial {
        j.out.errors.push(format!(
            "{name}: update comparison is incomplete; some contents could not be scanned"
        ));
    }
    Ok(Some(d))
}

fn update_hangar(
    http: &dyn Http,
    j: &mut Jar,
    mc: Option<&str>,
    limits: &Limits,
) -> Result<(), String> {
    let Some(Home::Hangar {
        slug,
        page,
        project,
        version,
        sha: official,
    }) = &j.home
    else {
        return Ok(());
    };
    let (slug, page, project, official) = (
        slug.clone(),
        page.clone(),
        project.clone(),
        official.clone(),
    );
    let current = version.clone().or_else(|| j.out.version.clone());
    // Without the installed release's date (its version wasn't found on Hangar), an older
    // release would look like an update.
    let Some(installed_at) = j.out.official.as_ref().and_then(|o| o.published.clone()) else {
        return Ok(());
    };
    let is_current = |name: &str| {
        current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(name))
    };
    // Hangar lists Minecraft versions for Paper builds only (proxy builds list proxy
    // versions), so --mc applies to plugins with a Paper build.
    let v: Value = match (mc, hangar_platforms(j).first()) {
        // The newest release that lists this Minecraft version for Paper.
        (Some(mc), Some(&"PAPER")) => {
            let url = format!(
                "{HANGAR}/projects/{}/versions?limit=1&offset=0&channel=Release&platform=PAPER&platformVersion={}",
                enc(&slug),
                enc(mc)
            );
            let Some(b) = http.get(&url)? else {
                return Ok(());
            };
            match parse::<Value>(&b, "Hangar versions")?["result"].get(0) {
                Some(v) => v.clone(),
                None => return Ok(()),
            }
        }
        (Some(_), None) => return Ok(()),
        _ => {
            let Some(b) = http.get(&format!("{HANGAR}/projects/{}/latestrelease", enc(&slug)))?
            else {
                return Ok(());
            };
            let latest = String::from_utf8_lossy(&b).trim().to_string();
            if latest.is_empty() || is_current(&latest) {
                return Ok(());
            }
            let Some(b) = http.get(&format!(
                "{HANGAR}/projects/{}/versions/{}",
                enc(&slug),
                enc(&latest)
            ))?
            else {
                return Ok(());
            };
            parse(&b, "Hangar version")?
        }
    };
    let Some(latest) = v["name"].as_str().map(str::to_string) else {
        return Ok(());
    };
    let all = ["PAPER", "WATERFALL", "VELOCITY"];
    // The jar's bytes, or the official file it was matched with, under another version name.
    let holds_installed = hangar_files(&v, &all)
        .iter()
        .any(|(_, _, sha)| *sha == j.out.sha256 || official.as_ref() == Some(sha));
    if is_current(&latest)
        || !newer(v["createdAt"].as_str(), Some(&installed_at))
        || holds_installed
    {
        return Ok(());
    }
    let mut source = Source {
        site: "hangar",
        project,
        url: format!("{page}/versions/{latest}"),
        version: Some(latest.clone()),
        published: v["createdAt"].as_str().map(str::to_string),
        file: None,
        download: None,
        diff: None,
    };
    if let Some((url, file, sha)) = hangar_files(&v, &hangar_platforms(j)).into_iter().next() {
        source.diff = download_diff(http, j, &url, &file, ("sha256", &sha), limits)?;
        source.file = Some(file);
        source.download = Some(url);
    }
    j.out.update = Some(source);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/../../fixtures/jars/{name}.jar",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// Answers from a table of URL prefixes; records every request.
    #[derive(Default)]
    struct Fake {
        routes: Vec<(String, Vec<u8>)>,
        log: RefCell<Vec<String>>,
    }

    impl Fake {
        fn route(mut self, prefix: &str, body: impl Into<Vec<u8>>) -> Self {
            self.routes.push((prefix.to_string(), body.into()));
            self
        }
        fn answer(&self, url: &str) -> Option<Vec<u8>> {
            self.log.borrow_mut().push(url.to_string());
            self.routes
                .iter()
                .filter(|(p, _)| url.starts_with(p.as_str()))
                .max_by_key(|(p, _)| p.len())
                .map(|(_, b)| b.clone())
        }
    }

    impl Http for Fake {
        fn get(&self, url: &str) -> Result<Option<Vec<u8>>, String> {
            Ok(self.answer(url))
        }
        fn post(&self, url: &str, _: &str) -> Result<Vec<u8>, String> {
            self.answer(url).ok_or_else(|| format!("{url}: HTTP 404"))
        }
    }

    fn sha512(b: &[u8]) -> String {
        hex(&Sha512::digest(b))
    }

    fn version(id: &str, number: &str, files: &[(&str, &[u8])]) -> Value {
        json!({
            "id": id, "project_id": "P1", "version_number": number, "version_type": "release",
            // Later versions are published later: 1.1.0 gives T110Z.
            "date_published": format!("2026-11-01T{}Z", number.replace('.', "")),
            "files": files.iter().map(|(name, b)| json!({
                "hashes": { "sha512": sha512(b), "sha1": "" }, "url": format!("https://cdn.example/{name}"),
                "filename": name, "primary": true
            })).collect::<Vec<_>>()
        })
    }

    fn project() -> Value {
        json!([{ "id": "P1", "slug": "fixture-inject", "title": "Fixture Inject", "project_type": "plugin" }])
    }

    fn one(http: &Fake, name: &str, bytes: Vec<u8>, updates: bool) -> Checked {
        run(
            http,
            vec![(name.to_string(), bytes)],
            None,
            updates,
            &Limits::default(),
        )
        .remove(0)
    }

    #[test]
    fn same_bytes_are_official() {
        let jar = fixture("inject-v1");
        let found = json!({ sha512(&jar): version("V1", "2.0.0", &[("inject-2.0.0.jar", &jar)]) });
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), found.to_string())
            .route(&format!("{MODRINTH}/projects"), project().to_string());
        let c = one(&http, "inject.jar", jar, false);
        assert_eq!(c.status, "official");
        let o = c.official.unwrap();
        assert_eq!(
            (o.site, o.project.as_str(), o.version.as_deref()),
            ("modrinth", "Fixture Inject", Some("2.0.0"))
        );
        assert!(
            c.errors.is_empty() && !http.log.borrow().iter().any(|u| u.contains("hangar")),
            "{:?}",
            http.log.borrow()
        );
    }

    #[test]
    fn injected_copy_differs_from_the_official_file() {
        let (official, local) = (fixture("inject-v1"), fixture("inject-v2"));
        let versions = json!([
            version("V1", "2.0.0", &[("Fixture-inject-2.0.0.jar", &official)]),
            version("V0", "1.0.0", &[("Fixture-inject-1.0.0.jar", b"x")])
        ]);
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(&format!("{MODRINTH}/search"), r#"{"hits":[]}"#)
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                versions.to_string(),
            )
            .route("https://cdn.example/Fixture-inject-2.0.0.jar", official);
        let c = one(&http, "Fixture-inject-2.0.0.jar", local.clone(), false);
        assert_eq!(c.status, "differs");
        assert!(c.alarming());
        // The official file is gone: reported, not taken for a jar with no official file.
        let gone = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(&format!("{MODRINTH}/search"), r#"{"hits":[]}"#)
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                versions.to_string(),
            );
        let g = one(&gone, "Fixture-inject-2.0.0.jar", local, false);
        assert_eq!(g.status, "not-found");
        assert!(
            g.errors.len() == 1 && g.errors[0].ends_with("Fixture-inject-2.0.0.jar: HTTP 404"),
            "{:?}",
            g.errors
        );
        let d = c.official.unwrap().diff.unwrap();
        let main = &d.code.changed_classes[0];
        assert_eq!(main.class, "fixture/Main");
        assert_eq!(main.methods_added, ["added$0()V"]);
        assert_eq!(main.methods_changed, ["onEnable()V"]);
        // The 1.0.0 file is never downloaded.
        assert!(!http.log.borrow().iter().any(|u| u.contains("1.0.0.jar")));
    }

    #[test]
    fn a_lookalike_project_is_not_the_official_file() {
        // Same version number, but a different plugin: name and main class don't match.
        let (other, local) = (fixture("diff-v1"), fixture("inject-v2"));
        let versions = json!([version("V1", "2.0.0", &[("other.jar", &other)])]);
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                versions.to_string(),
            )
            .route("https://cdn.example/other.jar", other);
        let c = one(&http, "x.jar", local, false);
        assert_eq!(c.status, "not-found");
        assert!(c.official.is_none());
        // It's in the project named like the plugin, so it's kept as the file it isn't.
        let u = c.unlike.unwrap();
        assert_eq!(
            (u.version.as_deref(), u.file.as_deref()),
            (Some("2.0.0"), Some("other.jar"))
        );
        assert!(u.diff.is_some());
    }

    #[test]
    fn a_failed_candidate_is_reported_even_when_another_matches() {
        let (official, local) = (fixture("inject-v1"), fixture("inject-v2"));
        // The file named like the jar is tried first, and its download is tampered with.
        let versions = json!([version(
            "V1",
            "2.0.0",
            &[
                ("x.jar", b"what the site lists"),
                ("Fixture-inject-2.0.0.jar", &official)
            ]
        )]);
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                versions.to_string(),
            )
            .route("https://cdn.example/x.jar", "something else")
            .route("https://cdn.example/Fixture-inject-2.0.0.jar", official);
        let c = one(&http, "x.jar", local, false);
        assert_eq!(c.status, "differs");
        assert!(
            c.errors.len() == 1 && c.errors[0].contains("doesn't match"),
            "{:?}",
            c.errors
        );
    }

    #[test]
    fn an_update_that_cant_be_downloaded_is_an_error() {
        let installed = fixture("diff-v1");
        let current = version("V1", "1.0.0", &[("f-1.0.0.jar", &installed)]);
        let newest = version("V2", "1.1.0", &[("f-1.1.0.jar", b"gone")]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string());
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none(), "{:?}", c.update);
        assert!(
            c.errors[0].ends_with("f-1.1.0.jar: HTTP 404"),
            "{:?}",
            c.errors
        );
    }

    #[test]
    fn a_tampered_download_is_an_error() {
        let local = fixture("inject-v2");
        let versions = json!([version("V1", "2.0.0", &[("f.jar", b"what the site lists")])]);
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                versions.to_string(),
            )
            .route("https://cdn.example/f.jar", "something else");
        let c = one(&http, "x.jar", local, false);
        assert_eq!(c.status, "not-found");
        assert!(c.errors[0].contains("doesn't match"), "{:?}", c.errors);
    }

    #[test]
    fn update_keeps_the_flavor_and_is_compared() {
        let (installed, update, other) =
            (fixture("diff-v1"), fixture("diff-v2"), fixture("inject-v1"));
        let current = version("V1", "1.0.0", &[("Fixture-Bukkit-1.0.0.jar", &installed)]);
        // The newest release only has a Paper build; the one before has the Bukkit build.
        let newest = version("V3", "1.2.0", &[("Fixture-Paper-1.2.0.jar", &other)]);
        let list = json!([
            newest,
            version(
                "V2",
                "1.1.0",
                &[
                    ("Fixture-Bukkit-1.1.0.jar", &update),
                    ("Fixture-Paper-1.1.0.jar", &other)
                ]
            ),
            current
        ]);
        let mut found = BTreeMap::new();
        found.insert(sha512(&installed), current.clone());
        let mut newest_map = BTreeMap::new();
        newest_map.insert(sha512(&installed), newest);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!(found).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!(newest_map).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(&format!("{MODRINTH}/project/P1/version"), list.to_string())
            .route("https://cdn.example/Fixture-Bukkit-1.1.0.jar", update);
        let c = one(&http, "fixture.jar", installed, true);
        assert_eq!(c.status, "official");
        let u = c.update.unwrap();
        assert_eq!(
            (u.version.as_deref(), u.file.as_deref()),
            (Some("1.1.0"), Some("Fixture-Bukkit-1.1.0.jar"))
        );
        let d = u.diff.unwrap();
        assert!(d.added.iter().any(|i| matches!(i, jarsnitch_core::Item::Finding { finding } if finding.rule == "code.load")));
    }

    #[test]
    fn another_flavor_of_the_same_version_is_no_update() {
        // Chunky: separate Bukkit and Paper versions, both numbered 1.0.0.
        let (installed, other) = (fixture("diff-v1"), fixture("inject-v1"));
        let current = version("V1", "1.0.0", &[("Fixture-Bukkit-1.0.0.jar", &installed)]);
        let paper = version("V2", "1.0.0", &[("Fixture-Paper-1.0.0.jar", &other)]);
        let list = json!([paper, current]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): paper }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(&format!("{MODRINTH}/project/P1/version"), list.to_string());
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.update);
    }

    #[test]
    fn renamed_files_get_an_update_and_a_parallel_flavor_doesnt() {
        let (installed, update) = (fixture("diff-v1"), fixture("diff-v2"));
        let current = version("V1", "1.0.0", &[("OldName-1.0.0.jar", &installed)]);
        let newest = version("V2", "1.1.0", &[("NewName-1.1.0.jar", &update)]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([newest, current]).to_string(),
            )
            .route("https://cdn.example/NewName-1.1.0.jar", update.clone());
        let c = one(&http, "f.jar", installed.clone(), true);
        let u = c.update.unwrap();
        assert_eq!(
            (u.file.as_deref(), u.diff.is_some()),
            (Some("NewName-1.1.0.jar"), true)
        );

        // The installed release also had a NewName build: that's another flavor, not a rename.
        let current = version(
            "V1",
            "1.0.0",
            &[
                ("OldName-1.0.0.jar", &installed),
                ("NewName-1.0.0.jar", b"other"),
            ],
        );
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([newest, current]).to_string(),
            );
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.update);
    }

    #[test]
    fn a_rename_preserves_explicit_platform_and_minecraft_markers() {
        let (installed, update) = (fixture("diff-v1"), fixture("diff-v2"));
        for (old_name, new_name, expected) in [
            ("Fixture-Bukkit-1.0.0.jar", "Fixture-Paper-1.1.0.jar", false),
            ("Fixture-1.0.0-1.8.jar", "NewBrand-1.1.0-1.21.jar", false),
            ("Fixture-Bukkit-1.0.0.jar", "NewBrand-1.1.0.jar", false),
            ("Fixture-1.0.0.jar", "NewBrand-Paper-1.1.0.jar", false),
            ("OldName-1.0.0.jar", "NewName-1.1.0.jar", true),
            (
                "OldName-Bukkit-1.0.0-1.8.jar",
                "NewName-Bukkit-1.1.0-1.8.jar",
                true,
            ),
        ] {
            let current = version("V1", "1.0.0", &[(old_name, &installed)]);
            let newest = version("V2", "1.1.0", &[(new_name, &update)]);
            let http = Fake::default()
                .route(
                    &format!("{MODRINTH}/version_files"),
                    json!({ sha512(&installed): current }).to_string(),
                )
                .route(
                    &format!("{MODRINTH}/version_files/update_individual"),
                    json!({ sha512(&installed): newest }).to_string(),
                )
                .route(&format!("{MODRINTH}/projects"), project().to_string())
                .route(
                    &format!("{MODRINTH}/project/P1/version"),
                    json!([newest, current]).to_string(),
                )
                .route(&format!("https://cdn.example/{new_name}"), update.clone());
            let c = one(&http, old_name, installed.clone(), true);
            assert!(c.errors.is_empty(), "{:?}", c.errors);
            assert_eq!(c.update.is_some(), expected, "{old_name} -> {new_name}");
            assert_eq!(
                http.log
                    .borrow()
                    .iter()
                    .any(|u| u.starts_with("https://cdn.example/")),
                expected
            );
        }
    }

    #[test]
    fn minecraft_markers_inside_release_versions_still_constrain_updates() {
        let (installed, update) = (fixture("diff-v1"), fixture("diff-v2"));
        for (new_name, mc, expected) in [
            ("Fixture", "1.21.1", false),
            ("NewName", "1.21.1", false),
            ("Fixture", "1.20.1", true),
            ("NewName", "1.20.1", true),
        ] {
            let old_name = "Fixture-1.0.0+mc1.20.1.jar";
            let new_name = format!("{new_name}-1.1.0+mc{mc}.jar");
            let current = version("V1", "1.0.0+mc1.20.1", &[(old_name, &installed)]);
            let newest = version("V2", &format!("1.1.0+mc{mc}"), &[(&new_name, &update)]);
            let http = Fake::default()
                .route(
                    &format!("{MODRINTH}/version_files"),
                    json!({ sha512(&installed): current }).to_string(),
                )
                .route(
                    &format!("{MODRINTH}/version_files/update_individual"),
                    json!({ sha512(&installed): newest }).to_string(),
                )
                .route(&format!("{MODRINTH}/projects"), project().to_string())
                .route(
                    &format!("{MODRINTH}/project/P1/version"),
                    json!([newest, current]).to_string(),
                )
                .route(&format!("https://cdn.example/{new_name}"), update.clone());
            let c = one(&http, old_name, installed.clone(), true);
            assert!(c.errors.is_empty(), "{:?}", c.errors);
            assert_eq!(c.update.is_some(), expected, "{new_name}");
        }
    }

    #[test]
    fn an_unreadable_update_is_an_error_even_when_its_hash_matches() {
        let installed = fixture("diff-v1");
        let current = version("V1", "1.0.0", &[("f-1.0.0.jar", &installed)]);
        let newest = version("V2", "1.1.0", &[("f-1.1.0.jar", b"not a jar")]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route("https://cdn.example/f-1.1.0.jar", "not a jar");
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none());
        assert!(
            c.errors.iter().any(|e| e.contains("readable jar")),
            "{:?}",
            c.errors
        );
    }

    #[test]
    fn a_partial_update_keeps_observed_high_findings_and_reports_incompleteness() {
        let (installed, update) = (fixture("sample-v1"), fixture("sample-v2"));
        let current = version("V1", "1.4.0", &[("SampleShop-1.4.0.jar", &installed)]);
        let newest = version("V2", "1.4.1", &[("SampleShop-1.4.1.jar", &update)]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route("https://cdn.example/SampleShop-1.4.1.jar", update);
        // Read the executable classes but stop before the final descriptor entry.
        let limits = Limits {
            max_entries: 10,
            ..Limits::default()
        };
        let c = run(
            &http,
            vec![("sample.jar".into(), installed)],
            None,
            true,
            &limits,
        )
        .remove(0);
        let d = c.update.as_ref().unwrap().diff.as_ref().unwrap();
        assert!(d.new.partial && d.adds_high());
        assert!(
            c.alarming(),
            "an incomplete check must retain observed High findings"
        );
        assert!(
            c.errors.iter().any(|e| e.contains("incomplete")),
            "{:?}",
            c.errors
        );
    }

    #[test]
    fn malformed_hangar_project_json_is_an_error() {
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), "[]")
            .route(&format!("{MODRINTH}/search"), r#"{"hits":[]}"#)
            .route(&format!("{HANGAR}/projects/Fixture-inject"), "not JSON");
        let c = one(&http, "inject.jar", fixture("inject-v1"), false);
        assert!(
            c.errors.iter().any(|e| e.contains("Hangar project")),
            "{:?}",
            c.errors
        );
    }

    /// `version()` published at `date`.
    fn dated(id: &str, number: &str, files: &[(&str, &[u8])], date: &str) -> Value {
        let mut v = version(id, number, files);
        v["date_published"] = json!(date);
        v
    }

    #[test]
    fn a_twin_uploaded_after_the_install_is_another_flavor() {
        // Bukkit and Paper builds as separate versions; the Paper twin of 1.0.0 went up 5 s later.
        let (installed, other) = (fixture("diff-v1"), fixture("inject-v1"));
        let current = dated(
            "V1",
            "1.0.0",
            &[("Fixture-Bukkit-1.0.0.jar", &installed)],
            "2026-11-01T10:00:00Z",
        );
        let twin = dated(
            "V2",
            "1.0.0",
            &[("Fixture-Paper-1.0.0.jar", b"twin")],
            "2026-11-01T10:00:05Z",
        );
        let newest = dated(
            "V3",
            "1.1.0",
            &[("Fixture-Paper-1.1.0.jar", &other)],
            "2026-11-10T00:00:00Z",
        );
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([newest, twin, current]).to_string(),
            )
            .route("https://cdn.example/Fixture-Paper-1.1.0.jar", other);
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.update);
    }

    #[test]
    fn builds_for_other_minecraft_versions_stay_apart() {
        let (installed, update, other) =
            (fixture("diff-v1"), fixture("diff-v2"), fixture("inject-v1"));
        let current = version(
            "V1",
            "1.0.0",
            &[
                ("Fixture-1.0.0-1.21.jar", b"new mc"),
                ("Fixture-1.0.0-1.8.jar", &installed),
            ],
        );
        let newest = version(
            "V2",
            "1.1.0",
            &[
                ("Fixture-1.1.0-1.21.jar", &other),
                ("Fixture-1.1.0-1.8.jar", &update),
            ],
        );
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route("https://cdn.example/Fixture-1.1.0-1.8.jar", update)
            .route("https://cdn.example/Fixture-1.1.0-1.21.jar", other);
        let c = one(&http, "f.jar", installed, true);
        assert_eq!(
            c.update.and_then(|u| u.file).as_deref(),
            Some("Fixture-1.1.0-1.8.jar")
        );
    }

    #[test]
    fn a_flavor_in_the_version_name_keeps_the_build() {
        // Versions named 5.29-paper and 5.29-spigot, the Spigot one uploaded 9 s later.
        let (old, latest, other) = (fixture("diff-v1"), fixture("diff-v2"), fixture("inject-v1"));
        let p28 = dated(
            "P28",
            "5.28-paper",
            &[("bluemap-5.28-paper.jar", &old)],
            "2026-09-01T10:00:00Z",
        );
        let s28 = dated(
            "S28",
            "5.28-spigot",
            &[("bluemap-5.28-spigot.jar", b"s28")],
            "2026-09-01T10:00:09Z",
        );
        let p29 = dated(
            "P29",
            "5.29-paper",
            &[("bluemap-5.29-paper.jar", &latest)],
            "2026-10-10T17:42:34.237568Z",
        );
        let s29 = dated(
            "S29",
            "5.29-spigot",
            &[("bluemap-5.29-spigot.jar", &other)],
            "2026-10-10T17:42:43.303260Z",
        );
        let list = json!([s29, p29, s28, p28]).to_string();
        let base = |installed: &[u8], current: &Value| {
            Fake::default()
                .route(
                    &format!("{MODRINTH}/version_files"),
                    json!({ sha512(installed): current }).to_string(),
                )
                .route(
                    &format!("{MODRINTH}/version_files/update_individual"),
                    json!({ sha512(installed): s29 }).to_string(),
                )
                .route(&format!("{MODRINTH}/projects"), project().to_string())
                .route(&format!("{MODRINTH}/project/P1/version"), list.clone())
                .route("https://cdn.example/bluemap-5.29-paper.jar", latest.clone())
                .route("https://cdn.example/bluemap-5.29-spigot.jar", other.clone())
        };
        let c = one(
            &base(&latest, &p29),
            "bluemap-5.29-paper.jar",
            latest.clone(),
            true,
        );
        assert!(
            c.update.is_none() && c.errors.is_empty(),
            "{:?}",
            c.update.map(|u| u.file)
        );
        let c = one(
            &base(&old, &p28),
            "bluemap-5.28-paper.jar",
            old.clone(),
            true,
        );
        assert_eq!(
            c.update.and_then(|u| u.file).as_deref(),
            Some("bluemap-5.29-paper.jar")
        );
    }

    #[test]
    fn the_same_version_again_isnt_an_update_but_a_rebuild_is() {
        let (installed, again) = (fixture("diff-v1"), fixture("diff-v2"));
        let current = dated(
            "V1",
            "1.0.0",
            &[("Plugin-1.0.0.jar", &installed)],
            "2026-10-01T00:00:00Z",
        );
        for (number, file, expect) in [
            ("1.0.0", "Plugin-1.0.0.jar", None),
            ("v1.0.0", "Plugin-1.0.0.jar", None),
            ("1.0.0-1", "Plugin-1.0.0-1.jar", Some("1.0.0-1")),
        ] {
            let newest = dated("V2", number, &[(file, &again)], "2026-10-01T00:05:00Z");
            let http = Fake::default()
                .route(
                    &format!("{MODRINTH}/version_files"),
                    json!({ sha512(&installed): current }).to_string(),
                )
                .route(
                    &format!("{MODRINTH}/version_files/update_individual"),
                    json!({ sha512(&installed): newest }).to_string(),
                )
                .route(&format!("{MODRINTH}/projects"), project().to_string())
                .route(
                    &format!("{MODRINTH}/project/P1/version"),
                    json!([newest, current]).to_string(),
                )
                .route(&format!("https://cdn.example/{file}"), again.clone());
            let c = one(&http, "Plugin-1.0.0.jar", installed.clone(), true);
            assert_eq!(
                c.update.and_then(|u| u.version).as_deref(),
                expect,
                "{number}"
            );
        }
    }

    #[test]
    fn with_mc_older_releases_still_guard_the_rename_rule() {
        // Bukkit and Paper builds of 1.0.0 list 1.20.4; 1.1.0 (Bukkit only) lists 1.21.4 only;
        // 1.2.0 (Paper only) lists both.
        let installed = fixture("diff-v1");
        let gv = |v: Value, g: &[&str]| {
            let mut v = v;
            v["game_versions"] = json!(g);
            v
        };
        let v1 = gv(
            dated(
                "V1",
                "1.0.0",
                &[("Fixture-Bukkit-1.0.0.jar", &installed)],
                "2026-11-01T10:00:00Z",
            ),
            &["1.20.4"],
        );
        let v2 = gv(
            dated(
                "V2",
                "1.0.0",
                &[("Fixture-Paper-1.0.0.jar", b"paper")],
                "2026-11-01T10:00:05Z",
            ),
            &["1.20.4"],
        );
        let v3 = gv(
            dated(
                "V3",
                "1.1.0",
                &[("Fixture-Bukkit-1.1.0.jar", b"bukkit")],
                "2026-11-05T00:00:00Z",
            ),
            &["1.21.4"],
        );
        let v4 = gv(
            dated(
                "V4",
                "1.2.0",
                &[("Fixture-Paper-1.2.0.jar", b"paper2")],
                "2026-11-10T00:00:00Z",
            ),
            &["1.20.4", "1.21.4"],
        );
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): v1 }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): v4 }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([v4, v3, v2, v1]).to_string(),
            );
        // --mc 1.20.4: 1.1.0 doesn't list it, and 1.2.0's Paper jar is the twin's flavor.
        let c = run(
            &http,
            vec![("Fixture-Bukkit-1.0.0.jar".into(), installed.clone())],
            Some("1.20.4"),
            true,
            &Limits::default(),
        )
        .remove(0);
        assert!(
            c.update.is_none() && c.errors.is_empty(),
            "{:?}",
            c.update.map(|u| u.file)
        );
        assert!(
            !http
                .log
                .borrow()
                .iter()
                .any(|u| u.contains("/version?") && u.contains("game_versions")),
            "{:?}",
            http.log.borrow()
        );
    }

    #[test]
    fn a_build_for_another_minecraft_version_isnt_the_update() {
        // 1.1.0 dropped the 1.8 build.
        let installed = fixture("diff-v1");
        let current = version(
            "V1",
            "1.0.0",
            &[
                ("Fixture-1.0.0-1.21.jar", b"new mc"),
                ("Fixture-1.0.0-1.8.jar", &installed),
            ],
        );
        let newest = version("V2", "1.1.0", &[("Fixture-1.1.0-1.21.jar", b"new mc 2")]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): newest }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([newest, current]).to_string(),
            );
        let c = one(&http, "Fixture-1.0.0-1.8.jar", installed, true);
        assert!(
            c.update.is_none() && c.errors.is_empty(),
            "{:?}",
            c.update.map(|u| u.file)
        );
    }

    #[test]
    fn dates_compare_to_the_fraction() {
        assert!(newer(
            Some("2026-10-01T00:00:00.900000Z"),
            Some("2026-10-01T00:00:00.100000Z")
        ));
        assert!(newer(
            Some("2026-10-01T00:00:00.1Z"),
            Some("2026-10-01T00:00:00Z")
        ));
        assert!(!newer(
            Some("2026-10-01T00:00:00Z"),
            Some("2026-10-01T00:00:00.000Z")
        ));
        assert!(!newer(
            Some("2026-09-30T23:59:59.999Z"),
            Some("2026-10-01T00:00:00Z")
        ));
        assert!(newer(None, Some("2026-10-01T00:00:00Z")) && newer(Some(""), Some("x")));
        assert!(!newer(
            Some("2026-10-01T00:00:00Zé"),
            Some("2026-10-01T00:00:01Z")
        ));
    }

    #[test]
    fn an_older_newest_release_is_no_update() {
        // Installed 1.1.0; the release Modrinth calls newest for it is an older 1.0.5.
        let (installed, older) = (fixture("diff-v2"), fixture("diff-v1"));
        let current = version("V2", "1.1.0", &[("f-1.1.0.jar", &installed)]);
        let back = version("V1", "1.0.5", &[("f-1.0.5.jar", &older)]);
        let http = Fake::default()
            .route(
                &format!("{MODRINTH}/version_files"),
                json!({ sha512(&installed): current }).to_string(),
            )
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                json!({ sha512(&installed): back }).to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string())
            .route(
                &format!("{MODRINTH}/project/P1/version"),
                json!([back, current]).to_string(),
            );
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.update);
    }

    #[test]
    fn up_to_date_has_no_update() {
        let installed = fixture("diff-v1");
        let current = version("V1", "1.0.0", &[("f.jar", &installed)]);
        let map = json!({ sha512(&installed): current });
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), map.to_string())
            .route(
                &format!("{MODRINTH}/version_files/update_individual"),
                map.to_string(),
            )
            .route(&format!("{MODRINTH}/projects"), project().to_string());
        let c = one(&http, "f.jar", installed, true);
        assert!(c.update.is_none() && c.errors.is_empty());
    }

    #[test]
    fn hangar_hash_match() {
        let jar = fixture("inject-v1");
        let sha = hex(&sha2::Sha256::digest(&jar));
        let project =
            json!({ "name": "Inject", "namespace": { "owner": "someone", "slug": "Inject" } });
        let version = json!({ "name": "2.0.0", "downloads": { "PAPER": { "fileInfo": { "name": "Inject.jar", "sha256Hash": sha }, "downloadUrl": "https://hangarcdn.example/Inject.jar" } } });
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(
                &format!("{HANGAR}/versions/hash/{sha}"),
                project.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Inject/versions/2.0.0"),
                version.to_string(),
            )
            .route(&format!("{HANGAR}/projects/Inject/latestrelease"), "2.0.0");
        let c = one(&http, "Inject.jar", jar, true);
        assert_eq!(c.status, "official");
        let o = c.official.unwrap();
        assert_eq!(
            (o.site, o.version.as_deref(), o.url.as_str()),
            (
                "hangar",
                Some("2.0.0"),
                "https://hangar.papermc.io/someone/Inject"
            )
        );
        assert!(c.update.is_none());
    }

    #[test]
    fn hangar_update_needs_the_installed_date() {
        let jar = fixture("inject-v1");
        let sha = hex(&sha2::Sha256::digest(&jar));
        let project =
            json!({ "name": "Inject", "namespace": { "owner": "someone", "slug": "Inject" } });
        let older = json!({ "name": "1.5.0", "createdAt": "2025-01-01T00:00:00.000Z", "downloads": { "PAPER": { "fileInfo": { "name": "Inject.jar", "sha256Hash": "00" }, "downloadUrl": "https://hangarcdn.example/Inject.jar" } } });
        // The jar's own version isn't on Hangar, so its date is unknown: no update.
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(
                &format!("{HANGAR}/versions/hash/{sha}"),
                project.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Inject/versions/1.5.0"),
                older.to_string(),
            )
            .route(&format!("{HANGAR}/projects/Inject/latestrelease"), "1.5.0");
        let c = one(&http, "Inject.jar", jar.clone(), true);
        assert_eq!(c.status, "official");
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.update);

        // The latest release holds these bytes under another version name: no update.
        let installed = json!({ "name": "2.0.0", "createdAt": "2026-01-01T00:00:00.000Z", "downloads": { "PAPER": { "fileInfo": { "name": "Inject.jar", "sha256Hash": sha }, "downloadUrl": "https://hangarcdn.example/Inject.jar" } } });
        let renamed = json!({ "name": "2.0.1", "createdAt": "2026-02-01T00:00:00.000Z", "downloads": { "PAPER": { "fileInfo": { "name": "Inject.jar", "sha256Hash": sha }, "downloadUrl": "https://hangarcdn.example/Inject.jar" } } });
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(
                &format!("{HANGAR}/versions/hash/{sha}"),
                project.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Inject/versions/2.0.0"),
                installed.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Inject/versions/2.0.1"),
                renamed.to_string(),
            )
            .route(&format!("{HANGAR}/projects/Inject/latestrelease"), "2.0.1");
        let c = one(&http, "Inject.jar", jar, true);
        assert!(c.update.is_none(), "{:?}", c.update);
    }

    #[test]
    fn hangar_update_isnt_the_official_file_again() {
        // A copy of the 2.0.0 file with other bytes (a zip comment), found on Hangar by name and
        // version: a later release that holds the 2.0.0 file isn't an update.
        let jar = fixture("inject-v1");
        let sha = hex(&sha2::Sha256::digest(&jar));
        let mut copy = jar.clone();
        let n = copy.len();
        assert_eq!(&copy[n - 22..n - 18], b"PK\x05\x06");
        copy[n - 2..].copy_from_slice(&4u16.to_le_bytes());
        copy.extend_from_slice(b"copy");
        let file = |n: &str| json!({ "PAPER": { "fileInfo": { "name": n, "sha256Hash": sha }, "downloadUrl": format!("https://hangarcdn.example/{n}") } });
        let project = json!({ "name": "Fixture-inject", "namespace": { "owner": "someone", "slug": "Fixture-inject" } });
        let installed = json!({ "name": "2.0.0", "createdAt": "2026-01-01T00:00:00.000Z", "downloads": file("Fixture-inject-2.0.0.jar") });
        let again = json!({ "name": "2.0.1", "createdAt": "2026-02-01T00:00:00.000Z", "downloads": file("Fixture-inject-2.0.1.jar") });
        let http = Fake::default()
            .route(&format!("{MODRINTH}/version_files"), "{}")
            .route(&format!("{MODRINTH}/projects"), "[]")
            .route(&format!("{MODRINTH}/search"), r#"{"hits":[]}"#)
            .route(
                &format!("{HANGAR}/projects/Fixture-inject"),
                project.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Fixture-inject/versions/2.0.0"),
                installed.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Fixture-inject/versions/2.0.1"),
                again.to_string(),
            )
            .route(
                &format!("{HANGAR}/projects/Fixture-inject/latestrelease"),
                "2.0.1",
            )
            .route("https://hangarcdn.example/", jar);
        let c = one(&http, "Fixture-inject-2.0.0.jar", copy, true);
        assert_eq!(c.status, "same-code", "{:?}", c.errors);
        assert!(
            c.update.is_none() && c.errors.is_empty(),
            "{:?} {:?}",
            c.update.map(|u| u.version),
            c.errors
        );
    }

    #[test]
    fn hangar_updates_follow_the_release_and_mc() {
        let (jar, newer_jar) = (fixture("inject-v1"), fixture("inject-v2"));
        let sha = hex(&sha2::Sha256::digest(&jar));
        let new_sha = hex(&sha2::Sha256::digest(&newer_jar));
        let project =
            json!({ "name": "Inject", "namespace": { "owner": "someone", "slug": "Inject" } });
        let installed = json!({ "name": "2.0.0", "createdAt": "2026-01-01T00:00:00.000Z", "downloads": { "PAPER": { "fileInfo": { "name": "Inject.jar", "sha256Hash": sha }, "downloadUrl": "https://hangarcdn.example/Inject.jar" } } });
        // A later release with a suffix is still a later release.
        let hotfix = json!({ "name": "2.0.0-hotfix", "createdAt": "2026-02-01T00:00:00.000Z", "downloads": { "PAPER": { "fileInfo": { "name": "Inject-hotfix.jar", "sha256Hash": new_sha }, "downloadUrl": "https://hangarcdn.example/Inject-hotfix.jar" } } });
        let base = || {
            Fake::default()
                .route(&format!("{MODRINTH}/version_files"), "{}")
                .route(
                    &format!("{HANGAR}/versions/hash/{sha}"),
                    project.to_string(),
                )
                .route(
                    &format!("{HANGAR}/projects/Inject/versions/2.0.0"),
                    installed.to_string(),
                )
                .route(
                    "https://hangarcdn.example/Inject-hotfix.jar",
                    newer_jar.clone(),
                )
        };
        let http = base()
            .route(
                &format!("{HANGAR}/projects/Inject/latestrelease"),
                "2.0.0-hotfix",
            )
            .route(
                &format!("{HANGAR}/projects/Inject/versions/2.0.0-hotfix"),
                hotfix.to_string(),
            );
        let c = one(&http, "Inject.jar", jar.clone(), true);
        let u = c.update.unwrap();
        assert_eq!(
            (u.version.as_deref(), u.diff.is_some()),
            (Some("2.0.0-hotfix"), true)
        );

        // With --mc, the newest release for that Minecraft version, not the latest one.
        let http = base()
            .route(&format!("{HANGAR}/projects/Inject/latestrelease"), "9.9.9")
            .route(
                &format!("{HANGAR}/projects/Inject/versions?"),
                json!({ "result": [hotfix] }).to_string(),
            );
        let c = run(
            &http,
            vec![("Inject.jar".into(), jar.clone())],
            Some("1.20.4"),
            true,
            &Limits::default(),
        )
        .remove(0);
        assert_eq!(
            c.update.and_then(|u| u.version).as_deref(),
            Some("2.0.0-hotfix")
        );
        assert!(
            http.log
                .borrow()
                .iter()
                .any(|u| u.contains("platform=PAPER&platformVersion=1.20.4")),
            "{:?}",
            http.log.borrow()
        );
        // None for it: no update.
        let http = base().route(
            &format!("{HANGAR}/projects/Inject/versions?"),
            r#"{"result":[]}"#,
        );
        let c = run(
            &http,
            vec![("Inject.jar".into(), jar)],
            Some("1.8.8"),
            true,
            &Limits::default(),
        )
        .remove(0);
        assert!(c.update.is_none() && c.errors.is_empty(), "{:?}", c.errors);
    }

    #[test]
    fn versions_and_flavors() {
        assert!(
            same_version("1.4.28", "1.4.28")
                && same_version("1.4.28", "v1.4.28")
                && same_version("1.4.28", "1.4.28-bukkit")
        );
        assert!(
            !same_version("1.4.28", "1.4.2")
                && !same_version("1.4.2", "1.4.28")
                && !same_version("1.4.28", "1.4.28.1")
        );
        for pre in [
            "2.0.0-SNAPSHOT",
            "2.0.0-beta.1",
            "2.0.0-rc1",
            "2.0.0-RC1",
            "2.0.0+build.5",
            "2.0.0-dev",
            "2.0.0-M1",
            "2.0.0-b45",
            "2.0.0-experimental",
        ] {
            assert!(
                !same_version("2.0.0", pre) && !same_version(pre, "2.0.0"),
                "{pre}"
            );
        }
        for flavor in [
            "1.0-premium",
            "1.0-sources",
            "1.0-architectury",
            "1.0-bukkit",
            "1.0 (paper)",
        ] {
            assert!(same_version("1.0", flavor), "{flavor}");
        }
        assert_eq!(
            stem("Chunky-Bukkit-1.4.28.jar"),
            stem("Chunky-Bukkit-1.4.36.jar")
        );
        assert!(
            exact_version("1.0.0", "V1.0.0")
                && !exact_version("1.0.0", "1.0.0-1")
                && !exact_version("", "")
        );
        assert_eq!(
            (base_version("5.29-paper"), base_version("v1.0.0+build.5")),
            ("5.29".into(), "1.0.0".into())
        );
        assert_ne!(
            stem("Chunky-Bukkit-1.4.28.jar"),
            stem("Chunky-Paper-1.4.36.jar")
        );
        assert_eq!(enc(r#"["a b"]"#), "%5B%22a%20b%22%5D");
    }
}
