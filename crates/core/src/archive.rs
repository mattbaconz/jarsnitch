//! Minimal zip reader. Reads the central directory the way the JVM's `ZipFile` does,
//! keeps duplicate names (the `zip` crate collapses them), and enforces size limits
//! before inflating.

use std::borrow::Cow;

pub struct Entry {
    /// Raw name, decoded lossily as UTF-8.
    pub name: String,
    pub flags: u16,
    pub method: u16,
    pub comp_size: u64,
    pub offset: u64,
}

pub enum ReadError {
    Limit(String),
    Unsupported(String),
    Corrupt(&'static str),
}

const EOCD: u32 = 0x0605_4b50;
const ZIP64_LOCATOR: u32 = 0x0706_4b50;
const ZIP64_EOCD: u32 = 0x0606_4b50;
const CEN: u32 = 0x0201_4b50;
const LOC: u32 = 0x0403_4b50;

fn u16_at(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(b: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// Returns the entries (at most `max_entries`) and whether the list was cut short by that cap.
pub fn entries(data: &[u8], max_entries: u64) -> Result<(Vec<Entry>, bool), &'static str> {
    let eocd = find_eocd(data).ok_or("no end-of-central-directory record")?;
    let mut count = u64::from(u16_at(data, eocd + 10).ok_or("truncated")?);
    let mut cd_size = u64::from(u32_at(data, eocd + 12).ok_or("truncated")?);
    let mut cd_offset = u64::from(u32_at(data, eocd + 16).ok_or("truncated")?);
    let mut end_pos = eocd as u64;

    if (count == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF)
        && eocd >= 20
        && u32_at(data, eocd - 20) == Some(ZIP64_LOCATOR)
        && let Some(z64) = u64_at(data, eocd - 20 + 8).and_then(|p| usize::try_from(p).ok())
        && u32_at(data, z64) == Some(ZIP64_EOCD)
    {
        count = u64_at(data, z64 + 32).ok_or("truncated zip64 record")?;
        cd_size = u64_at(data, z64 + 40).ok_or("truncated zip64 record")?;
        cd_offset = u64_at(data, z64 + 48).ok_or("truncated zip64 record")?;
        end_pos = z64 as u64;
    }

    // Data prepended to the archive (a launcher stub, say) shifts every offset. The JVM
    // corrects for it the same way: the directory ends where the end record starts.
    let cd_start = end_pos.checked_sub(cd_size).ok_or("central directory out of bounds")?;
    let base = cd_start.checked_sub(cd_offset).ok_or("central directory out of bounds")?;

    let mut out = Vec::new();
    let mut pos = usize::try_from(cd_start).map_err(|_| "central directory out of bounds")?;
    let mut truncated = false;
    for _ in 0..count {
        if out.len() as u64 >= max_entries {
            truncated = true;
            break;
        }
        if u32_at(data, pos) != Some(CEN) {
            return Err("bad central directory entry");
        }
        let h = data.get(pos..pos + 46).ok_or("truncated central directory")?;
        let flags = u16_at(h, 8).unwrap_or(0);
        let method = u16_at(h, 10).unwrap_or(0);
        let mut comp_size = u64::from(u32_at(h, 20).unwrap_or(0));
        let mut size = u64::from(u32_at(h, 24).unwrap_or(0));
        let name_len = usize::from(u16_at(h, 28).unwrap_or(0));
        let extra_len = usize::from(u16_at(h, 30).unwrap_or(0));
        let comment_len = usize::from(u16_at(h, 32).unwrap_or(0));
        let mut offset = u64::from(u32_at(h, 42).unwrap_or(0));
        let name = data.get(pos + 46..pos + 46 + name_len).ok_or("truncated entry name")?;
        let extra = data.get(pos + 46 + name_len..pos + 46 + name_len + extra_len).unwrap_or(&[]);
        read_zip64_extra(extra, &mut size, &mut comp_size, &mut offset);
        out.push(Entry {
            name: String::from_utf8_lossy(name).into_owned(),
            flags,
            method,
            comp_size,
            offset: offset.saturating_add(base),
        });
        pos += 46 + name_len + extra_len + comment_len;
    }
    Ok((out, truncated))
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let lowest = data.len().saturating_sub(22 + 0xFFFF);
    (lowest..=data.len() - 22).rev().find(|&i| {
        u32_at(data, i) == Some(EOCD)
            && u16_at(data, i + 20).is_some_and(|comment| i + 22 + usize::from(comment) <= data.len())
    })
}

fn read_zip64_extra(mut extra: &[u8], size: &mut u64, comp_size: &mut u64, offset: &mut u64) {
    while extra.len() >= 4 {
        let id = u16_at(extra, 0).unwrap_or(0);
        let len = usize::from(u16_at(extra, 2).unwrap_or(0));
        let Some(body) = extra.get(4..4 + len) else { return };
        if id == 0x0001 {
            let mut at = 0;
            for field in [size, comp_size, offset] {
                if *field == 0xFFFF_FFFF {
                    let Some(v) = u64_at(body, at) else { return };
                    *field = v;
                    at += 8;
                }
            }
            return;
        }
        extra = &extra[4 + len..];
    }
}

/// Returns the entry's uncompressed bytes, refusing to produce more than `limit` bytes or
/// to exceed `max_ratio` for entries over 1 MiB.
pub fn read<'a>(data: &'a [u8], e: &Entry, limit: u64, max_ratio: u64) -> Result<Cow<'a, [u8]>, ReadError> {
    if e.flags & 1 != 0 {
        return Err(ReadError::Unsupported("encrypted entry".into()));
    }
    let loc = usize::try_from(e.offset).map_err(|_| ReadError::Corrupt("entry offset out of bounds"))?;
    if u32_at(data, loc) != Some(LOC) {
        return Err(ReadError::Corrupt("bad local header"));
    }
    let name_len = usize::from(u16_at(data, loc + 26).ok_or(ReadError::Corrupt("truncated local header"))?);
    let extra_len = usize::from(u16_at(data, loc + 28).ok_or(ReadError::Corrupt("truncated local header"))?);
    let start = loc + 30 + name_len + extra_len;
    let comp = usize::try_from(e.comp_size).map_err(|_| ReadError::Corrupt("entry size out of bounds"))?;
    let raw = data
        .get(start..start.saturating_add(comp))
        .ok_or(ReadError::Corrupt("entry data out of bounds"))?;
    match e.method {
        0 => {
            if raw.len() as u64 > limit {
                return Err(ReadError::Limit(format!("entry is larger than {limit} bytes")));
            }
            Ok(Cow::Borrowed(raw))
        }
        8 => {
            let ratio_cap = (raw.len() as u64).saturating_mul(max_ratio).max(1 << 20);
            let cap = limit.min(ratio_cap);
            match miniz_oxide::inflate::decompress_to_vec_with_limit(raw, usize::try_from(cap).unwrap_or(usize::MAX)) {
                Ok(v) => Ok(Cow::Owned(v)),
                Err(err) if err.status == miniz_oxide::inflate::TINFLStatus::HasMoreOutput => {
                    if cap < limit {
                        Err(ReadError::Limit(format!("compression ratio above {max_ratio}:1")))
                    } else {
                        Err(ReadError::Limit(format!("entry is larger than {limit} bytes")))
                    }
                }
                Err(_) => Err(ReadError::Corrupt("bad deflate data")),
            }
        }
        m => Err(ReadError::Unsupported(format!("compression method {m}"))),
    }
}

/// Cleans an entry name for display. Returns the clean name and whether the original
/// could point outside the archive when extracted.
pub fn clean_name(name: &str) -> (String, bool) {
    let has_drive = name.len() >= 2 && name.as_bytes()[1] == b':' && name.as_bytes()[0].is_ascii_alphabetic();
    let unsafe_path = name.starts_with('/')
        || name.contains('\\')
        || has_drive
        || name.split(['/', '\\']).any(|seg| seg == "..");
    if !unsafe_path {
        return (name.to_string(), false);
    }
    let trimmed = if has_drive { &name[2..] } else { name };
    let parts: Vec<&str> = trimmed
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect();
    let mut clean = parts.join("/");
    if name.ends_with('/') || name.ends_with('\\') {
        clean.push('/');
    }
    (clean, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_names() {
        assert_eq!(clean_name("a/b/C.class"), ("a/b/C.class".into(), false));
        assert_eq!(clean_name("../../start.sh"), ("start.sh".into(), true));
        assert_eq!(clean_name("/etc/x"), ("etc/x".into(), true));
        assert_eq!(clean_name("C:\\x\\y.txt"), ("x/y.txt".into(), true));
        assert_eq!(clean_name("a/../b"), ("a/b".into(), true));
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(entries(b"", 10).is_err());
        assert!(entries(b"PK\x05\x06", 10).is_err());
        let mut fake = vec![0u8; 100];
        fake.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        fake.extend_from_slice(&[0xff; 18]);
        assert!(entries(&fake, 10).is_err());
    }
}
