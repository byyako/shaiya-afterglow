//! The server's version as an extension's `server` range sees it (`docs/plans/extensions-split.md` section 3.1).

use semver::{Prerelease, Version, VersionReq};

/// The version this build was made as (`0.4.0-dev` on `main` between releases).
pub const BUILD: &str = env!("CARGO_PKG_VERSION");

/// This build's version for an extension's `server` range: the workspace version without its pre-release part, so a `0.4.0-dev`
/// build of `main` satisfies `>=0.4.0, <0.5.0` as the 0.4.0 release will.
pub fn running() -> Version {
    let mut version = Version::parse(BUILD).expect("the crate version is a semantic version");
    version.pre = Prerelease::EMPTY;
    version
}

/// Parses a `server` range as `extension.toml` writes it (`">=0.4.0, <0.5.0"`).
pub fn parse_range(text: &str) -> Result<VersionReq, String> {
    VersionReq::parse(text).map_err(|e| format!("server {text:?} is not a version range (like \">=0.4.0, <0.5.0\"): {e}"))
}

/// `None` when `range` takes this build; otherwise why not, naming the extension.
pub fn refusal(extension: &str, extension_version: &str, range: &VersionReq) -> Option<String> {
    refusal_for(extension, extension_version, range, &running(), BUILD)
}

fn refusal_for(extension: &str, extension_version: &str, range: &VersionReq, running: &Version, build: &str) -> Option<String> {
    (!range.matches(running)).then(|| format!("{extension} {extension_version} needs server {range}; this is {build}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_build_counts_as_its_release() {
        let range = parse_range(">=0.4.0, <0.5.0").unwrap();
        let dev = Version::parse("0.4.0").unwrap();
        assert_eq!(refusal_for("pets", "1.0.0", &range, &dev, "0.4.0-dev"), None);
        assert_eq!(
            refusal_for("pets", "1.0.0", &range, &Version::parse("0.3.0").unwrap(), "0.3.0").as_deref(),
            Some("pets 1.0.0 needs server >=0.4.0, <0.5.0; this is 0.3.0")
        );
        assert!(refusal_for("pets", "1.0.0", &range, &Version::parse("0.5.0").unwrap(), "0.5.0").is_some());
        assert_eq!(running().pre, Prerelease::EMPTY);
    }

    #[test]
    fn a_range_that_is_not_one_is_refused_with_an_example() {
        let error = parse_range("0.4 or later").unwrap_err();
        assert!(error.contains("\"0.4 or later\" is not a version range (like \">=0.4.0, <0.5.0\")"), "{error}");
    }
}
