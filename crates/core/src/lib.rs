//! jarsnitch core: reads a Minecraft plugin jar and reports what its code can do.
//!
//! No I/O, no network, no clock, no randomness. The same bytes always give the
//! same report. Analyzed code is parsed, never loaded or run.

#![forbid(unsafe_code)]

mod archive;
mod class;
mod diff;
mod meta;
mod reach;
mod report;
mod rules;
mod scan;
mod strings;

pub use diff::{Change, Diff, Item, Key, OldNew, Side};
pub use report::*;
pub use rules::{RULES, Rule, rule};

/// Caps that keep hostile archives from exhausting memory or time.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Uncompressed bytes across the whole tree (top-level jar, nested jars, decoded payloads).
    pub max_total_bytes: u64,
    /// Uncompressed bytes for a single entry.
    pub max_entry_bytes: u64,
    /// Entries across the whole tree.
    pub max_entries: u64,
    /// Highest compression ratio accepted for entries over 1 MiB.
    pub max_ratio: u64,
    /// Nesting depth; the top-level jar is depth 0.
    pub max_depth: u32,
    /// Largest text resource scanned for strings (also caps descriptor files).
    pub max_text_bytes: usize,
    /// Encoded payloads fully decoded per scan.
    pub max_decoded: u32,
    /// Bytes produced by decoding payloads per scan.
    pub max_decoded_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_total_bytes: 512 << 20,
            max_entry_bytes: 128 << 20,
            max_entries: 100_000,
            max_ratio: 200,
            max_depth: 3,
            max_text_bytes: 1 << 20,
            max_decoded: 1_000,
            max_decoded_bytes: 16 << 20,
        }
    }
}

/// Scans one jar. Never panics on bad input: problems become warnings in the report.
pub fn scan(name: &str, bytes: &[u8], limits: &Limits) -> Report {
    scan::run(name, bytes, limits).0
}

/// Compares two versions of a jar, leading with what `new` adds. Each is scanned with
/// `limits`; a partial scan on either side adds a `diff.partial` warning.
pub fn diff(old_name: &str, old: &[u8], new_name: &str, new: &[u8], limits: &Limits) -> Diff {
    diff::diff(scan::run(old_name, old, limits), scan::run(new_name, new, limits))
}
