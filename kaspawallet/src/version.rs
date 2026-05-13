//! `version` subcommand. Prints the binary's semantic version on
//! its own line.

/// Compile-time version pulled from the crate's `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Print the binary's version line on stdout in the form
/// `kaspawallet v<semver>`.
pub fn print() {
    println!("kaspawallet v{VERSION}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_matches_pkg_semver() {
        // The parity-matrix framing regex
        // (`^kaspawallet v\d+\.\d+\.\d+(\S*)?\s*$`) must accept
        // our version line. Implemented dependency-free since the
        // shape is a single anchored pattern.
        let line = format!("kaspawallet v{VERSION}");
        assert!(matches_framing(&line), "version line failed framing match: {line:?}");
    }

    fn matches_framing(s: &str) -> bool {
        // Anchored shape: `kaspawallet v<major>.<minor>.<patch><tail>`
        // where major/minor are numeric and patch starts with at
        // least one digit; the tail may contain dots (pre-release
        // identifiers like `-rc.3` are valid SemVer).
        let Some(rest) = s.strip_prefix("kaspawallet v") else { return false };
        let rest = rest.trim_end();
        let mut parts = rest.splitn(3, '.');
        let major = parts.next();
        let minor = parts.next();
        let patch_and_tail = parts.next();
        if major.is_none() || minor.is_none() || patch_and_tail.is_none() {
            return false;
        }
        if !major.unwrap().bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        if !minor.unwrap().bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        let patch = patch_and_tail.unwrap();
        let mut digits = 0usize;
        for b in patch.bytes() {
            if b.is_ascii_digit() {
                digits += 1;
            } else {
                break;
            }
        }
        digits > 0
    }
}
