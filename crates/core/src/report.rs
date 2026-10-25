use serde::Serialize;

pub const SCHEMA_VERSION: u32 = 1;
pub const DISCLAIMER: &str =
    "This shows what the code can do, not proof of what it will do. A clean report isn't a guarantee.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Notable,
    High,
}

impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Severity::Info => "INFO",
            Severity::Notable => "NOTABLE",
            Severity::High => "HIGH",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub tool_version: String,
    pub file: FileInfo,
    /// True when a limit stopped part of the scan. A partial report is not a clean one.
    pub partial: bool,
    pub plugins: Vec<PluginInfo>,
    pub libraries: Vec<Library>,
    pub nested: Vec<Nested>,
    pub obfuscation: Obfuscation,
    pub findings: Vec<Finding>,
    pub warnings: Vec<Warning>,
    pub stats: Stats,
    pub disclaimer: String,
}

impl Report {
    pub fn has_high(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::High)
    }

    /// The file couldn't be opened as a zip at all.
    pub fn is_unreadable(&self) -> bool {
        unreadable(&self.warnings)
    }
}

pub(crate) fn unreadable(warnings: &[Warning]) -> bool {
    warnings.iter().any(|w| w.code == "zip.unreadable" && w.location.is_empty())
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct PluginInfo {
    pub platform: String,
    pub descriptor: String,
    pub name: Option<String>,
    pub version: Option<String>,
    pub main: Option<String>,
    pub bootstrapper: Option<String>,
    pub loader: Option<String>,
    pub commands: Vec<String>,
    pub permissions: Vec<String>,
    pub folia_supported: bool,
    pub maven_libraries: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Library {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub location: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Nested {
    pub path: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Obfuscation {
    pub score: u32,
    pub short_names: f64,
    pub default_package: f64,
    pub encoded_strings: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub title: String,
    pub why: String,
    pub legit_uses: String,
    /// Host, IP, webhook id, path, class name or hash for value-carrying rules.
    pub value: Option<String>,
    /// First 50 locations, sorted.
    pub evidence: Vec<Evidence>,
    pub evidence_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Evidence {
    /// Class internal name or entry path, with nested jars as `lib.jar!/a/b/C`.
    pub location: String,
    /// Method name and descriptor, when the evidence is inside a method.
    pub method: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub code: String,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Stats {
    pub entries: u64,
    pub classes: u64,
    pub uncompressed_bytes: u64,
}
