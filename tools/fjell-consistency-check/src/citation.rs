//! What a citation in a document may look like (RFC-0.33-004 D4, E-052).
//!
//! The book is published, and a relative path that leaves it
//! (`../../../rfcs/done/…`) resolves on disk and 404s on the site. So a document
//! may cite a repository file by **absolute repository URL**,
//! `{repository_url}/blob/main/<path>` (or `/tree/main/` for a directory), and
//! `standards-mapping` and `evidence` — which used to resolve every citation as a
//! filesystem path and therefore refused exactly that — turn it into the
//! repo-relative path and check it **as they check a relative one**: it must exist
//! on disk, and for `evidence` it must have its provenance sidecar.
//!
//! One definition of "the repository URL": the `git-repository-url` the book
//! already publishes (`docs/book.toml`). Two subchecks, each its own instrument,
//! share this one small function rather than each carrying a copy of the rule.

/// Where the repository URL is read from.
pub const BOOK_TOML: &str = "docs/book.toml";

/// How a link in a document is to be resolved.
#[derive(Debug, PartialEq, Eq)]
pub enum Citation<'a> {
    /// A relative path: resolved against the document's directory, as before.
    Relative(&'a str),
    /// An absolute URL into this repository's default branch: the path from the
    /// repository root, fragment and query removed.
    Repository(String),
    /// Any other absolute URL: **not** a citation this repository can check.
    Foreign(&'a str),
}

/// Classify one link target.
pub fn classify<'a>(link: &'a str, repository_url: &str) -> Citation<'a> {
    let is_absolute = link.contains("://");
    if !is_absolute {
        return Citation::Relative(link);
    }
    let base = repository_url.trim_end_matches('/');
    for kind in ["blob", "tree"] {
        let prefix = format!("{base}/{kind}/main/");
        if let Some(rest) = link.strip_prefix(&prefix) {
            let path = rest.split(['#', '?']).next().unwrap_or("");
            if !path.is_empty() {
                return Citation::Repository(path.to_string());
            }
        }
    }
    Citation::Foreign(link)
}

/// The `git-repository-url` from `docs/book.toml`'s text.
pub fn repository_url_from(book_toml: &str) -> Option<String> {
    for line in book_toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("git-repository-url") {
            let v = rest.trim_start().strip_prefix('=')?.trim();
            let v = v.trim_matches('"');
            if v.starts_with("https://") {
                return Some(v.trim_end_matches('/').to_string());
            }
        }
    }
    None
}

/// The URL for the running check, or a message saying why there is none.
pub fn repository_url() -> Result<String, String> {
    let src =
        std::fs::read_to_string(BOOK_TOML).map_err(|e| format!("cannot read {BOOK_TOML}: {e}"))?;
    repository_url_from(&src)
        .ok_or_else(|| format!("{BOOK_TOML} has no `git-repository-url = \"https://…\"`"))
}

/// The URL fixtures in this crate's tests use.
#[cfg(test)]
pub const TEST_REPOSITORY_URL: &str = "https://example.test/org/repo";

#[cfg(test)]
mod tests {
    use super::*;

    const R: &str = "https://github.com/nabbisen/fjell-os";

    #[test]
    fn a_relative_path_is_relative() {
        assert_eq!(
            classify("../../rfcs/x.md", R),
            Citation::Relative("../../rfcs/x.md")
        );
    }

    #[test]
    fn a_blob_url_becomes_its_repository_path() {
        assert_eq!(
            classify(
                "https://github.com/nabbisen/fjell-os/blob/main/rfcs/done/x.md#sec",
                R
            ),
            Citation::Repository("rfcs/done/x.md".into())
        );
        assert_eq!(
            classify(
                "https://github.com/nabbisen/fjell-os/tree/main/tests/evidence",
                R
            ),
            Citation::Repository("tests/evidence".into())
        );
    }

    #[test]
    fn a_trailing_slash_on_the_configured_url_does_not_matter() {
        assert_eq!(
            classify(
                "https://github.com/nabbisen/fjell-os/blob/main/a",
                "https://github.com/nabbisen/fjell-os/"
            ),
            Citation::Repository("a".into())
        );
    }

    /// Only *this* repository, only its default branch: another repository, a
    /// pinned commit, or the bare repository page is not a checkable citation.
    #[test]
    fn everything_else_absolute_is_foreign() {
        for l in [
            "https://github.com/other/repo/blob/main/a.md",
            "https://github.com/nabbisen/fjell-os/blob/0123abc/a.md",
            "https://github.com/nabbisen/fjell-os",
            "https://github.com/nabbisen/fjell-os/blob/main/",
            "https://example.com/x",
        ] {
            assert_eq!(classify(l, R), Citation::Foreign(l), "{l}");
        }
    }

    #[test]
    fn the_url_is_read_from_the_book_config() {
        let toml = "[output.html]\ngit-repository-url = \"https://github.com/nabbisen/fjell-os\"\n";
        assert_eq!(repository_url_from(toml).as_deref(), Some(R));
        assert_eq!(repository_url_from("[book]\ntitle = \"x\"\n"), None);
        assert_eq!(
            repository_url_from("git-repository-url = \"ftp://x\"\n"),
            None
        );
        // and the real file has one
        let real = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/book.toml"),
        )
        .unwrap();
        assert_eq!(repository_url_from(&real).as_deref(), Some(R));
    }
}
