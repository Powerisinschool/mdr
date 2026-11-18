use crate::Options;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn is_external_url(url: &str) -> bool {
    url.starts_with("http://") || url.starts_with("https://")
}

/// Downloads and caches an external image into `images/`.
/// If fetching fails, logs a warning with the URL and falls back to the direct URL.
pub fn localize_url(
    url: &str,
    alt: &str,
    variant: Option<&str>,
    opts: &Options,
    referenced_images: &mut HashSet<String>,
) -> String {
    if !is_external_url(url) {
        return url.to_string();
    }

    let slug = slugify(alt);
    let hash = hash_url(url);
    let stem = match variant {
        Some(v) => format!("{slug}-{v}-{hash}"),
        None => format!("{slug}-{hash}"),
    };

    if !opts.refresh_images {
        if let Some(existing_name) = find_cached_file(&opts.images_dir, &stem) {
            referenced_images.insert(existing_name.clone());
            return format!("images/{existing_name}");
        }
    }

    if opts.no_fetch {
        let fallback = format!("{stem}.{}", guess_ext_from_url(url).unwrap_or("svg"));
        referenced_images.insert(fallback.clone());
        return format!("images/{fallback}");
    }

    if let Err(err) = fs::create_dir_all(&opts.images_dir) {
        eprintln!(
            "Warning: image resource ({url}) failed to load (could not create {}: {err}); direct URL was used.",
            opts.images_dir.display()
        );
        return url.to_string();
    }

    let tmp_body = env::temp_dir().join(format!("mdr_dl_{stem}.bin"));
    let tmp_hdr = env::temp_dir().join(format!("mdr_dl_{stem}.hdr"));

    let status = Command::new("curl")
        .args([
            "-fsSL",
            "--retry",
            "2",
            "--connect-timeout",
            "10",
            "--max-time",
            "20",
            "-D",
            tmp_hdr.to_str().unwrap_or("/tmp/mdr.hdr"),
            url,
            "-o",
            tmp_body.to_str().unwrap_or("/tmp/mdr.bin"),
        ])
        .status();

    let fetched_ok = matches!(status, Ok(s) if s.success());

    if !fetched_ok {
        let _ = fs::remove_file(&tmp_body);
        let _ = fs::remove_file(&tmp_hdr);
        eprintln!("Warning: image resource ({url}) failed to load; direct URL was used.");
        return url.to_string();
    }

    let headers = fs::read_to_string(&tmp_hdr).unwrap_or_default();
    let ext = guess_ext_from_url(url)
        .or_else(|| guess_ext_from_headers(&headers))
        .unwrap_or("svg");

    let filename = format!("{stem}.{ext}");
    let dest = opts.images_dir.join(&filename);

    if let Err(err) = fs::copy(&tmp_body, &dest) {
        let _ = fs::remove_file(&tmp_body);
        let _ = fs::remove_file(&tmp_hdr);
        eprintln!(
            "Warning: image resource ({url}) failed to load (write error: {err}); direct URL was used."
        );
        return url.to_string();
    }

    let _ = fs::remove_file(&tmp_body);
    let _ = fs::remove_file(&tmp_hdr);

    referenced_images.insert(filename.clone());
    format!("images/{filename}")
}

pub fn prune_unused_images(dir: &Path, referenced: &HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !referenced.contains(&name) {
                let _ = fs::remove_file(path);
            }
        }
    }
}

fn find_cached_file(dir: &Path, stem: &str) -> Option<String> {
    let prefix = format!("{stem}.");
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&prefix) {
            return Some(name);
        }
    }
    None
}

fn slugify(input: &str) -> String {
    let normalized = input
        .replace("++", "pp")
        .replace('#', "sharp")
        .to_ascii_lowercase();

    let mut out = String::new();
    let mut last_dash = false;
    for ch in normalized.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        "image".to_string()
    } else {
        trimmed.to_string()
    }
}

fn hash_url(url: &str) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in url.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", (hash >> 32) ^ (hash & 0xffff_ffff))
}

fn guess_ext_from_url(url: &str) -> Option<&'static str> {
    let path_part = url.split(['?', '#']).next()?;
    let (_, ext) = path_part.rsplit_once('.')?;
    match ext.to_ascii_lowercase().as_str() {
        "svg" => Some("svg"),
        "png" => Some("png"),
        "jpg" | "jpeg" => Some("jpg"),
        "webp" => Some("webp"),
        "gif" => Some("gif"),
        _ => None,
    }
}

fn guess_ext_from_headers(headers: &str) -> Option<&'static str> {
    for line in headers.lines().rev() {
        let lower = line.to_ascii_lowercase();
        if let Some(ct) = lower.strip_prefix("content-type:") {
            if ct.contains("svg") {
                return Some("svg");
            } else if ct.contains("png") {
                return Some("png");
            } else if ct.contains("jpeg") || ct.contains("jpg") {
                return Some("jpg");
            } else if ct.contains("webp") {
                return Some("webp");
            } else if ct.contains("gif") {
                return Some("gif");
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir =
                env::temp_dir().join(format!("mdr_test_{}_{}_{}", label, std::process::id(), id));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).expect("failed to create temp test dir");
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn test_opts(images_dir: &Path, no_fetch: bool) -> Options {
        Options {
            input_path: PathBuf::from("README.mdr"),
            output_path: PathBuf::from("README.md"),
            images_dir: images_dir.to_path_buf(),
            refresh_images: false,
            no_fetch,
        }
    }

    #[test]
    fn slugifies_special_tech_names() {
        assert_eq!(slugify("C++"), "cpp");
        assert_eq!(slugify("C#"), "csharp");
        assert_eq!(slugify("Tolu's GitHub Stats"), "tolu-s-github-stats");
        assert_eq!(slugify("!!!"), "image");
    }

    #[test]
    fn detects_extensions_from_urls_and_headers() {
        assert_eq!(
            guess_ext_from_url("https://example.com/banner.png?size=large#top"),
            Some("png")
        );
        assert_eq!(
            guess_ext_from_url("https://img.shields.io/badge/Rust-161B22"),
            None
        );

        let redirect_headers = "\
HTTP/1.1 302 Found\r\n\
Content-Type: text/html\r\n\
\r\n\
HTTP/1.1 200 OK\r\n\
Content-Type: image/svg+xml; charset=utf-8\r\n";

        assert_eq!(guess_ext_from_headers(redirect_headers), Some("svg"));
    }

    #[test]
    fn reuses_cached_asset_without_network() {
        let tmp = TempDir::new("cache_hit");
        let url = "https://img.shields.io/badge/Rust-161B22";
        let hash = hash_url(url);
        let cached_name = format!("rust-dark-{hash}.svg");
        fs::write(tmp.path().join(&cached_name), "<svg></svg>").unwrap();

        let opts = test_opts(tmp.path(), false);
        let mut referenced = HashSet::new();
        let resolved = localize_url(url, "Rust", Some("dark"), &opts, &mut referenced);

        assert_eq!(resolved, format!("images/{cached_name}"));
        assert!(referenced.contains(&cached_name));
    }

    #[test]
    fn falls_back_to_direct_url_when_fetch_fails() {
        let tmp = TempDir::new("fetch_fail");
        let opts = test_opts(tmp.path(), false);
        let mut referenced = HashSet::new();
        let bad_url = "http://127.0.0.1:0/unreachable-badge.svg";

        let resolved = localize_url(bad_url, "Broken", None, &opts, &mut referenced);

        assert_eq!(resolved, bad_url);
        assert!(referenced.is_empty());
    }

    #[test]
    fn prunes_only_unreferenced_images() {
        let tmp = TempDir::new("prune");
        let keep_file = tmp.path().join("rust-dark-12345678.svg");
        let stale_file = tmp.path().join("old-badge-deadbeef.svg");
        fs::write(&keep_file, "<svg/>").unwrap();
        fs::write(&stale_file, "<svg/>").unwrap();

        let mut referenced = HashSet::new();
        referenced.insert("rust-dark-12345678.svg".to_string());

        prune_unused_images(tmp.path(), &referenced);

        assert!(keep_file.exists());
        assert!(!stale_file.exists());
    }
}
