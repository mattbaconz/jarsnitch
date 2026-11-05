//! String heuristics: URLs and hosts, IPs, webhooks, encoded payloads.

pub enum UrlKind {
    Host(String),
    Ip(String),
    Discord { id: String },
    Telegram { id: String },
}

pub struct Url {
    pub kind: UrlKind,
    /// The URL as found, with any token redacted, cut to 200 characters.
    pub text: String,
}

const SCHEMES: &[&str] = &["http", "https", "ws", "wss", "ftp"];

const DISCORD_HOSTS: &[&str] = &[
    "discord.com",
    "discordapp.com",
    "ptb.discord.com",
    "canary.discord.com",
    "ptb.discordapp.com",
    "canary.discordapp.com",
];

/// TLDs accepted for bare hostnames. Kept short on purpose: TLDs that are also common
/// words (info, top, app, sh, md, ...) turn permission nodes and file names into hosts.
const BARE_TLDS: &[&str] = &[
    "com", "net", "org", "io", "gg", "me", "dev", "xyz", "eu", "us", "uk", "de", "fr", "nl", "pl", "ru",
    "ua", "br", "es", "it", "se", "no", "fi", "dk", "cz", "ro", "hu", "tr", "ca", "au", "jp", "kr", "cn",
    "in", "su", "tk", "cc",
];

pub fn cut(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s.to_string(),
    }
}

fn token_end(c: char) -> bool {
    c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '`' | '\\' | '|' | '^' | '{' | '}')
}

/// Every URL with a known scheme in `s`.
pub fn urls(s: &str) -> Vec<Url> {
    let mut out = Vec::new();
    if !s.contains("://") {
        return out;
    }
    let b = s.as_bytes();
    let mut from = 0;
    while let Some(i) = s[from..].find("://") {
        let at = from + i;
        from = at + 3;
        let mut start = at;
        while start > 0 && b[start - 1].is_ascii_alphabetic() && at - start < 5 {
            start -= 1;
        }
        let scheme = s[start..at].to_ascii_lowercase();
        if !SCHEMES.contains(&scheme.as_str()) {
            continue;
        }
        let rest = &s[at + 3..];
        let full = &rest[..rest.find(token_end).unwrap_or(rest.len())];
        let auth_end = full.find(['/', '?', '#']).unwrap_or(full.len());
        let auth = full[..auth_end].rsplit_once('@').map_or(&full[..auth_end], |(_, h)| h);
        let path = &full[auth_end..];
        let text = cut(&format!("{scheme}://{full}"), 200);
        if let Some(v6) = auth.strip_prefix('[') {
            let addr = v6.split(']').next().unwrap_or("").to_ascii_lowercase();
            if is_public_v6(&addr) {
                out.push(Url { kind: UrlKind::Ip(addr), text });
            }
            continue;
        }
        let host = auth.split(':').next().unwrap_or("").trim_end_matches('.').to_ascii_lowercase();
        if let Some(ip) = parse_v4(&host) {
            if is_public_v4(ip) {
                out.push(Url { kind: UrlKind::Ip(host), text });
            }
            continue;
        }
        if !valid_host(&host) {
            continue;
        }
        if DISCORD_HOSTS.contains(&host.as_str())
            && let Some((id, token)) = discord_webhook(path)
        {
            let text = format!("{scheme}://{host}/api/webhooks/{id}/{}…", &token[..4]);
            out.push(Url { kind: UrlKind::Discord { id: id.to_string() }, text });
            continue;
        }
        if host == "api.telegram.org"
            && let Some((id, token)) = telegram_bot(path)
        {
            let text = format!("{scheme}://{host}/bot{id}:{}…", &token[..4]);
            out.push(Url { kind: UrlKind::Telegram { id: id.to_string() }, text });
            continue;
        }
        out.push(Url { kind: UrlKind::Host(host), text });
    }
    out
}

fn valid_host(h: &str) -> bool {
    h.len() <= 253
        && h.contains('.')
        && h.split('.').all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && !l.starts_with('-')
                && !l.ends_with('-')
                && l.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        })
}

fn take_while(s: &str, f: impl Fn(u8) -> bool) -> (&str, &str) {
    let n = s.bytes().take_while(|&c| f(c)).count();
    s.split_at(n)
}

fn token_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-' || c == b'_'
}

/// `/api[/vN]/webhooks/<id>/<token>` with both parts present.
fn discord_webhook(path: &str) -> Option<(&str, &str)> {
    let mut p = path.strip_prefix("/api/")?;
    if let Some(v) = p.strip_prefix('v') {
        let (digits, rest) = take_while(v, |c| c.is_ascii_digit());
        if !digits.is_empty() {
            p = rest.strip_prefix('/')?;
        }
    }
    let p = p.strip_prefix("webhooks/")?;
    let (id, rest) = take_while(p, |c| c.is_ascii_digit());
    let (token, _) = take_while(rest.strip_prefix('/')?, token_char);
    ((17..=20).contains(&id.len()) && (60..=80).contains(&token.len())).then_some((id, token))
}

/// `/bot<id>:<token>`.
fn telegram_bot(path: &str) -> Option<(&str, &str)> {
    let p = path.strip_prefix("/bot")?;
    let (id, rest) = take_while(p, |c| c.is_ascii_digit());
    let (token, _) = take_while(rest.strip_prefix(':')?, token_char);
    ((6..=12).contains(&id.len()) && (30..=50).contains(&token.len())).then_some((id, token))
}

/// A string that is nothing but a hostname, like `play.example.net`. Java package and
/// class names (`net.example.Foo`, `org.bukkit`) are rejected: they start with a TLD
/// or contain uppercase letters.
pub fn bare_host(s: &str) -> Option<&str> {
    let t = s.trim();
    if t.len() < 4 || !valid_host(t) || t.contains('_') {
        return None;
    }
    let labels: Vec<&str> = t.split('.').collect();
    let (first, last) = (labels[0], labels[labels.len() - 1]);
    let ok = BARE_TLDS.contains(&last)
        && !BARE_TLDS.contains(&first)
        && labels[labels.len() - 2].len() >= 2
        && !labels.iter().all(|l| l.bytes().all(|c| c.is_ascii_digit()));
    ok.then_some(t)
}

fn parse_v4(s: &str) -> Option<[u8; 4]> {
    let mut out = [0u8; 4];
    let mut parts = s.split('.');
    for slot in &mut out {
        let p = parts.next()?;
        if p.is_empty() || p.len() > 3 || (p.len() > 1 && p.starts_with('0')) || !p.bytes().all(|c| c.is_ascii_digit()) {
            return None;
        }
        *slot = p.parse().ok()?;
    }
    parts.next().is_none().then_some(out)
}

pub fn is_public_v4([a, b, c, _]: [u8; 4]) -> bool {
    !(a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 198 && (b == 18 || b == 19)))
}

fn is_public_v6(a: &str) -> bool {
    !a.is_empty()
        && a.contains(':')
        && a.bytes().all(|c| c.is_ascii_hexdigit() || c == b':' || c == b'.')
        && !(a == "::" || a == "::1" || a.starts_with("fe8") || a.starts_with("fe9") || a.starts_with("fea")
            || a.starts_with("feb") || a.starts_with("fc") || a.starts_with("fd"))
}

/// Public IPv4 literals that stand alone in `s` (not part of a version number like
/// `1.2.3.4.5` or `v1.2.3.4`, not after `rv:`). Without a port, these look like
/// addresses but usually aren't, so they're skipped: Maven versions after `:`, addresses
/// ending in `.0`, ASN.1 object ids (`2.5.29.14`: first part 1 or 2, second below 40),
/// and section or version numbers with every part below 16 (`4.2.1.10`).
pub fn ipv4s(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        // A sentence-ending dot isn't part of the address.
        let mut end = i;
        while end > start && b[end - 1] == b'.' {
            end -= 1;
        }
        let blocked = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
        let before_ok = start == 0 || !blocked(b[start - 1]);
        let after_ok = i == b.len() || !blocked(b[i]);
        let rv = start >= 3 && s[..start].to_ascii_lowercase().ends_with("rv:");
        let port = end == i && b.get(i) == Some(&b':') && b.get(i + 1).is_some_and(u8::is_ascii_digit);
        if before_ok && after_ok && !rv
            && let Some(ip) = parse_v4(&s[start..end])
            && is_public_v4(ip)
            && (port || !(start > 0 && b[start - 1] == b':' || not_an_address(ip)))
        {
            out.push(s[start..end].to_string());
        }
    }
    out
}

fn not_an_address([a, b, c, d]: [u8; 4]) -> bool {
    d == 0 || (a <= 2 && b < 40) || [a, b, c, d].iter().all(|&x| x < 16)
}

pub enum Payload {
    Class,
    Zip,
}

pub enum Encoding {
    Base64,
    Hex,
}

fn b64_value(c: u8) -> Option<u32> {
    Some(u32::from(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' | b'-' => 62,
        b'/' | b'_' => 63,
        _ => return None,
    }))
}

fn is_b64(s: &str) -> bool {
    let body = s.trim_end_matches('=');
    s.len() - body.len() <= 2 && body.len() % 4 != 1 && body.bytes().all(|c| b64_value(c).is_some())
}

fn is_hex(s: &str) -> bool {
    s.len().is_multiple_of(2) && s.bytes().all(|c| c.is_ascii_hexdigit())
}

/// Decodes standard or URL-safe base64, padding optional. `None` on a bad character.
pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let body = s.trim_end_matches('=');
    let mut out = Vec::with_capacity(body.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in body.bytes() {
        acc = (acc << 6) | b64_value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    (0..b.len() / 2)
        .map(|i| u8::from_str_radix(std::str::from_utf8(&b[2 * i..2 * i + 2]).ok()?, 16).ok())
        .collect()
}

fn magic(prefix: &[u8]) -> Option<Payload> {
    if crate::class::is_class(prefix) {
        Some(Payload::Class)
    } else if prefix.starts_with(b"PK\x03\x04") {
        Some(Payload::Zip)
    } else {
        None
    }
}

/// Whether `s` is a base64 or hex string of 64+ characters that decodes to a class file
/// or a zip. Only a short prefix is decoded here; the caller decides whether the full
/// decode fits its budget.
pub fn payload(s: &str) -> Option<(Payload, Encoding)> {
    let s = s.trim();
    if s.len() < 64 {
        return None;
    }
    if is_hex(s)
        && let Some(kind) = hex_decode(&s[..20]).and_then(|p| magic(&p))
    {
        return Some((kind, Encoding::Hex));
    }
    if is_b64(s) {
        return b64_decode(&s[..16]).and_then(|p| magic(&p)).map(|k| (k, Encoding::Base64));
    }
    None
}

/// `bytes` as text, if it's valid UTF-8 without control characters.
pub fn readable(bytes: Vec<u8>) -> Option<String> {
    String::from_utf8(bytes).ok().filter(|t| !t.chars().any(char::is_control))
}

/// The address a base64 or hex literal hides, as the decoded text. Skin textures are
/// base64 JSON with a Mojang URL in nearly every plugin that draws heads; they don't count.
pub fn hidden_address(s: &str) -> Option<String> {
    let s = s.trim();
    if !(16..=4096).contains(&s.len()) {
        return None;
    }
    let bytes = if is_hex(s) {
        hex_decode(s)?
    } else if is_b64(s) {
        b64_decode(s)?
    } else {
        return None;
    };
    let text = readable(bytes)?;
    let address = !urls(&text).is_empty() || !ipv4s(&text).is_empty();
    (address && !text.contains("textures.minecraft.net")).then_some(text)
}

/// A lowercase name such as `org.bstats` that isn't an address. Libraries write their own
/// package name as bytes so that shading doesn't rewrite it (bStats does this in nearly every
/// plugin), and a word like `run` says nothing alone. Reflection still sees these.
pub fn is_package_name(s: &str) -> bool {
    s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'_'))
        && bare_host(s).is_none()
        && ipv4s(s).is_empty()
}

/// Files that hold logins on someone's machine, in lowercase with forward slashes. Mentioned
/// in public write-ups of credential stealers, such as
/// https://github.com/trigram-mrp/fractureiser/blob/main/docs/tech.md
const LOGIN_FILES: &[&str] = &[
    "local storage/leveldb",
    "/login data",
    "/network/cookies",
    "cookies.sqlite",
    "key4.db",
    "launcher_accounts.json",
    "launcher_msa_credentials",
    "microsoft_accounts.json",
    "wallet.dat",
    ".ssh/id_",
    ".aws/credentials",
    ".git-credentials",
    "/etc/shadow",
];

/// The login file `s` refers to, if any. A key name counts alone only when it's the whole
/// string, as in `Path.of(home, ".ssh", "id_rsa")`: `id_RSASSA-PSS` is a certificate term.
pub fn login_file(s: &str) -> Option<&'static str> {
    if matches!(s, "id_rsa" | "id_ed25519") {
        return Some(".ssh/id_");
    }
    if s.len() < 6 {
        return None;
    }
    let s = s.to_ascii_lowercase().replace('\\', "/");
    LOGIN_FILES.iter().find(|f| s.contains(**f)).copied()
}

pub fn decode(s: &str, enc: &Encoding) -> Option<Vec<u8>> {
    match enc {
        Encoding::Base64 => b64_decode(s.trim()),
        Encoding::Hex => hex_decode(s.trim()),
    }
}

/// For the obfuscation score: a literal that looks like encoded or encrypted data
/// rather than text. Base64 that decodes to readable text (skin textures are base64
/// JSON) doesn't count.
pub fn looks_encoded(s: &str) -> bool {
    let n = s.chars().count();
    if n < 20 {
        return false;
    }
    if is_hex(s) {
        return n >= 32;
    }
    let has = |f: fn(&u8) -> bool| s.as_bytes().iter().any(f);
    let longest_lower_run = s
        .split(|c: char| !c.is_ascii_lowercase())
        .map(str::len)
        .max()
        .unwrap_or(0);
    if is_b64(s) && has(u8::is_ascii_uppercase) && has(u8::is_ascii_lowercase) && has(u8::is_ascii_digit) && longest_lower_run < 7 {
        let prefix = b64_decode(&s[..s.len().min(24)]).unwrap_or_default();
        return prefix.iter().any(|&c| (c < 0x20 && !b"\n\r\t".contains(&c)) || c > 0x7e);
    }
    n >= 32 && entropy(s) > 4.8
}

fn entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    let mut n = 0.0;
    for c in s.chars() {
        *counts.entry(c).or_insert(0.0) += 1.0;
        n += 1.0;
    }
    counts.values().map(|&k: &f64| -(k / n) * (k / n).log2()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(s: &str) -> Vec<String> {
        urls(s)
            .into_iter()
            .map(|u| match u.kind {
                UrlKind::Host(h) | UrlKind::Ip(h) => h,
                UrlKind::Discord { id } => format!("discord:{id}"),
                UrlKind::Telegram { id } => format!("telegram:{id}"),
            })
            .collect()
    }

    #[test]
    fn url_hosts() {
        assert_eq!(hosts("see https://API.Example.invalid:8443/x?y=1 now"), ["api.example.invalid"]);
        assert_eq!(hosts("ftp://user:pw@files.example.com/a"), ["files.example.com"]);
        assert_eq!(hosts("https://%s/api and http://{host}/x and http://localhost/"), Vec::<String>::new());
        assert_eq!(hosts("http://127.0.0.1/ http://203.0.113.9/x"), ["203.0.113.9"]);
        assert_eq!(hosts("http://[2001:db8::1]:8080/ http://[::1]/"), ["2001:db8::1"]);
        assert_eq!(hosts("jar:file:/x and mailto:a@b.c"), Vec::<String>::new());
    }

    #[test]
    fn webhooks_need_id_and_token() {
        let token = "x".repeat(68);
        let hook = format!("https://discord.com/api/webhooks/000000000000000000/{token}");
        assert_eq!(hosts(&hook), ["discord:000000000000000000"]);
        assert!(urls(&hook)[0].text.ends_with("/xxxx…"));
        assert_eq!(hosts(&format!("https://ptb.discordapp.com/api/v10/webhooks/123456789012345678/{token}")), ["discord:123456789012345678"]);
        assert_eq!(hosts("https://discord.com/api/webhooks/"), ["discord.com"]);
        let bot = format!("https://api.telegram.org/bot000000000:{}/sendMessage", "A".repeat(35));
        assert_eq!(hosts(&bot), ["telegram:000000000"]);
        assert!(!urls(&bot)[0].text.contains(&"A".repeat(5)));
    }

    #[test]
    fn bare_hosts() {
        assert_eq!(bare_host("play.example.com"), Some("play.example.com"));
        assert_eq!(bare_host("  mc.example.net "), Some("mc.example.net"));
        for no in ["net.minecraft.server", "org.bukkit", "com.example.Main", "config.yml", "myplugin.info",
                   "start.sh", "1.20.4", "myplugin.admin", "README.md", "a.com"] {
            assert_eq!(bare_host(no), None, "{no}");
        }
    }

    #[test]
    fn ips() {
        assert_eq!(ipv4s("connect to 203.0.113.7:25565 now"), ["203.0.113.7"]);
        for no in ["10.0.0.1", "192.168.1.1", "127.0.0.1", "1.2.3.4.5", "v1.2.3.4", "rv:1.2.3.4", "255.255.255.0",
                   "01.2.3.4", "1.2.3", "0.0.0.0", "1.2.3.4-SNAPSHOT", "org.xerial:sqlite-jdbc:3.46.1.0",
                   "org.xerial:sqlite-jdbc:3.42.0.1", "2.5.29.14", "1.3.101.75", "see sec 8.9.5.2, bullet 4", "4.2.1.10"] {
            assert!(ipv4s(no).is_empty(), "{no}");
        }
        assert_eq!(ipv4s("88.66.44.22,203.0.113.7"), ["88.66.44.22", "203.0.113.7"]);
        assert_eq!(ipv4s("1.2.3.4:25565 and 8.8.8.8:53"), ["1.2.3.4", "8.8.8.8"], "a port makes it an address");
    }

    #[test]
    fn payloads() {
        let class = [0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 61, 0, 3, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut bytes = class.repeat(4);
        bytes.truncate(60);
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        assert!(matches!(payload(&hex), Some((Payload::Class, Encoding::Hex))));
        assert_eq!(decode(&hex, &Encoding::Hex).unwrap(), bytes);
        let b64 = "UEsDBBQAAAAIAAAAIQAAAAAAAAAAAAAAAAAJAAAAbWFuaWZlc3QuTUZ4eHh4eHh4eHh4eA==";
        assert!(matches!(payload(b64), Some((Payload::Zip, Encoding::Base64))));
        assert!(payload(&"eyJ0ZXh0dXJlcyI6".repeat(5)).is_none());
        assert_eq!(b64_decode("aGVsbG8").unwrap(), b"hello");
        assert_eq!(b64_decode("aGVsbG8=").unwrap(), b"hello");
    }

    #[test]
    fn encoded_looking() {
        assert!(looks_encoded("q8Zr3kV1xP0wLm9tYbN2cH5gJ7dF4sA6"));
        assert!(looks_encoded("9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"));
        assert!(!looks_encoded("eyJ0ZXh0dXJlcyI6eyJTS0lOIjp7InVybCI6Imh0dHA6Ly90"));
        assert!(!looks_encoded("Welcome to the server, enjoy your stay!"));
        assert!(!looks_encoded("playerDataManagerInstance2"));
    }
}
