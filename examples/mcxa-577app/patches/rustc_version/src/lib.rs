use std::process::Command;
use std::env;

pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Debug)]
pub struct Error(String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn version() -> Result<Version, Error> {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let output = Command::new(&rustc)
        .arg("--version")
        .output()
        .map_err(|e| Error(e.to_string()))?;
    let s = std::str::from_utf8(&output.stdout).map_err(|e| Error(e.to_string()))?;
    // "rustc 1.93.0 (..." or "rustc 1.93.0-nightly (..."
    let ver = s.split_whitespace().nth(1).ok_or_else(|| Error("bad output".into()))?;
    let ver = ver.split('-').next().unwrap_or(ver);
    let mut parts = ver.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error("bad major".into()))?;
    let minor = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error("bad minor".into()))?;
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok(Version { major, minor, patch })
}
