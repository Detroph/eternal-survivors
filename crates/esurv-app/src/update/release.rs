//! The release feed: GitHub's answer for the latest release of the game, read down to what the
//! updater needs. A release is offered only with both of its files: the archive
//! `eternal-survivors-<version>.zip` and its checksum `eternal-survivors-<version>.zip.sha256`.

use esurv_setup::parse_version;

/// The feed the player builds ask: the public repository's latest release (GitHub leaves drafts
/// and pre-releases out of it).
pub const FEED: &str = "https://api.github.com/repos/Detroph/eternal-survivors/releases/latest";

/// The start of a release archive's name; the version and `.zip` follow.
pub const ARCHIVE_PREFIX: &str = "eternal-survivors-";

/// A version: major, minor, patch.
pub type Version = (u32, u32, u32);

/// One file of a release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    /// Where it downloads from (GitHub's `browser_download_url`, which redirects to its store).
    pub url: String,
    /// Its size in bytes, as the feed says (0 when it says none).
    pub size: u64,
}

/// The latest release, as the updater offers it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The archive, which holds the setup.
    pub zip: Asset,
    /// The archive's checksum file.
    pub sha256: Asset,
}

/// A release tag's version: `esurv-v0.5.0`, `v0.5.0` or `0.5.0`.
pub fn version_of_tag(tag: &str) -> Option<Version> {
    let t = tag.trim();
    let t = t
        .get(..6)
        .filter(|p| p.eq_ignore_ascii_case("esurv-"))
        .map_or(t, |_| &t[6..]);
    parse_version(t)
}

/// A version as the game writes it: `v0.5.0`.
pub fn version_text(v: Version) -> String {
    format!("v{}.{}.{}", v.0, v.1, v.2)
}

/// The archive's name for a version: `eternal-survivors-v0.5.0.zip`.
pub fn archive_name(v: Version) -> String {
    format!("{ARCHIVE_PREFIX}{}.zip", version_text(v))
}

/// The checksum file's name for a version: the archive's, then `.sha256`.
pub fn checksum_name(v: Version) -> String {
    format!("{}.sha256", archive_name(v))
}

/// Whether `release` is newer than the version installed (`VERSION.txt`'s text); an installed
/// version that does not read as one is never updated.
pub fn is_newer(installed: &str, release: Version) -> bool {
    parse_version(installed).is_some_and(|v| release > v)
}

/// The release in the feed's JSON: its tag's version and its two files, `Err` saying what is
/// missing.
pub fn parse_release(json: &str) -> Result<Release, String> {
    let doc: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("the release feed is not JSON: {e}"))?;
    let tag = doc
        .get("tag_name")
        .and_then(|t| t.as_str())
        .ok_or("the release feed names no tag")?;
    let version = version_of_tag(tag).ok_or_else(|| format!("the tag {tag:?} is no version"))?;
    let assets = doc
        .get("assets")
        .and_then(|a| a.as_array())
        .map(Vec::as_slice)
        .unwrap_or_default();
    let find = |name: &str| -> Result<Asset, String> {
        assets
            .iter()
            .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name))
            .and_then(|a| {
                Some(Asset {
                    name: name.to_owned(),
                    url: a.get("browser_download_url")?.as_str()?.to_owned(),
                    size: a.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                })
            })
            .ok_or_else(|| format!("the release {tag} has no {name}"))
    };
    Ok(Release {
        version,
        zip: find(&archive_name(version))?,
        sha256: find(&checksum_name(version))?,
    })
}

/// A feed answer as GitHub writes it, cut down to the fields read (and a few that are not), each
/// asset downloading from `base`.
#[cfg(test)]
pub(crate) fn test_feed_json(tag: &str, base: &str, assets: &[&str]) -> String {
    let assets: Vec<String> = assets
        .iter()
        .map(|name| {
            format!(
                r#"{{"name": "{name}", "size": 1234, "content_type": "application/zip",
                    "browser_download_url": "{base}/{name}"}}"#
            )
        })
        .collect();
    format!(
        r#"{{"tag_name": "{tag}", "name": "Eternal Survivors {tag}", "draft": false,
            "prerelease": false, "assets": [{}]}}"#,
        assets.join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_json(tag: &str, base: &str, assets: &[&str]) -> String {
        test_feed_json(tag, base, assets)
    }

    #[test]
    fn versions_compare_as_numbers_part_by_part() {
        assert!(is_newer("v0.4.0", (0, 5, 0)));
        assert!(is_newer("v0.4.0\r\n", (0, 4, 1)));
        assert!(is_newer("v0.9.0", (0, 10, 0)), "10 is more than 9");
        assert!(is_newer("v0.4.9", (1, 0, 0)));
        assert!(!is_newer("v0.4.0", (0, 4, 0)), "the same version");
        assert!(!is_newer("v0.5.0", (0, 4, 9)), "an older one");
        assert!(!is_newer("", (9, 9, 9)), "no installed version: no update");
        assert!(!is_newer("dev", (9, 9, 9)));
    }

    #[test]
    fn a_tag_reads_with_or_without_its_prefix() {
        assert_eq!(version_of_tag("esurv-v0.5.0"), Some((0, 5, 0)));
        assert_eq!(version_of_tag("ESURV-v0.5.0"), Some((0, 5, 0)));
        assert_eq!(version_of_tag("v0.12.3"), Some((0, 12, 3)));
        assert_eq!(version_of_tag("0.5.0"), Some((0, 5, 0)));
        assert_eq!(version_of_tag("esurv-v0.5.0-rc1"), None);
        assert_eq!(version_of_tag("esurv"), None);
        assert_eq!(version_text((0, 5, 0)), "v0.5.0");
        assert_eq!(archive_name((0, 5, 0)), "eternal-survivors-v0.5.0.zip");
        assert_eq!(checksum_name((0, 5, 0)), "eternal-survivors-v0.5.0.zip.sha256");
    }

    #[test]
    fn the_feed_gives_the_version_and_both_files() {
        let json = feed_json(
            "esurv-v0.5.0",
            "https://example.test/dl",
            &["eternal-survivors-v0.5.0.zip", "eternal-survivors-v0.5.0.zip.sha256", "notes.txt"],
        );
        let r = parse_release(&json).unwrap();
        assert_eq!(r.version, (0, 5, 0));
        assert_eq!(r.zip.name, "eternal-survivors-v0.5.0.zip");
        assert_eq!(r.zip.url, "https://example.test/dl/eternal-survivors-v0.5.0.zip");
        assert_eq!(r.zip.size, 1234);
        assert_eq!(r.sha256.url, "https://example.test/dl/eternal-survivors-v0.5.0.zip.sha256");
    }

    #[test]
    fn a_release_without_its_checksum_or_its_archive_is_not_offered() {
        let base = "https://example.test/dl";
        let no_sum = feed_json("esurv-v0.5.0", base, &["eternal-survivors-v0.5.0.zip"]);
        assert!(parse_release(&no_sum).unwrap_err().contains(".sha256"));
        // an archive of another version is not this release's
        let other = feed_json(
            "esurv-v0.5.0",
            base,
            &["eternal-survivors-v0.4.0.zip", "eternal-survivors-v0.4.0.zip.sha256"],
        );
        assert!(parse_release(&other).is_err());
        assert!(parse_release(&feed_json("nightly", base, &[])).is_err());
        assert!(parse_release("<html>rate limited</html>").is_err());
        assert!(parse_release(r#"{"message": "API rate limit exceeded"}"#).is_err());
    }
}
