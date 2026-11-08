//! Bounded local URI-list parsing, shared with the application regression tests.
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

pub(super) const MAX_BYTES: usize = 1024 * 1024;
const MAX_FILES: usize = 256;

pub(super) fn local_paths(bytes: &[u8]) -> Option<Vec<PathBuf>> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let mut paths = Vec::new();
    for line in std::str::from_utf8(bytes).ok()?.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let uri = line.strip_prefix("file://")?;
        let path = if uri.starts_with('/') {
            uri
        } else {
            let local = uri.strip_prefix("localhost")?;
            if !local.starts_with('/') {
                return None;
            }
            local
        };
        // Fragments, queries and malformed percent escapes must not become paths.
        if path.contains(['?', '#']) {
            return None;
        }
        let encoded = path.as_bytes();
        let mut i = 0;
        while i < encoded.len() {
            if encoded[i] == b'%' {
                if i + 2 >= encoded.len()
                    || !encoded[i + 1].is_ascii_hexdigit()
                    || !encoded[i + 2].is_ascii_hexdigit()
                {
                    return None;
                }
                i += 3;
            } else {
                i += 1;
            }
        }
        let decoded: Vec<u8> = percent_encoding::percent_decode(encoded).collect();
        if decoded.contains(&0) || !decoded.starts_with(b"/") {
            return None;
        }
        let path = PathBuf::from(std::ffi::OsString::from_vec(decoded));
        if !paths.contains(&path) {
            if paths.len() >= MAX_FILES {
                return None;
            }
            paths.push(path);
        }
    }
    Some(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_local_unicode_and_deduplicates() {
        assert_eq!(local_paths(b"# comment\r\nfile:///tmp/%C5%BD%20%25%23.pdf\r\nfile://localhost/tmp/%C5%BD%20%25%23.pdf\r\n"),
                   Some(vec![PathBuf::from("/tmp/Ž %#.pdf")]));
    }
    #[test]
    fn rejects_remote_malformed_and_non_file_uris() {
        for uri in [
            "https://example.invalid/a.pdf",
            "file://remote/tmp/a.pdf",
            "file:///tmp/%",
            "file:///tmp/%GG.pdf",
            "file:///tmp/%00.pdf",
            "file:///tmp/a.pdf?query",
            "file:///tmp/a.pdf#fragment",
            "file://localhost.evil/tmp/a.pdf",
        ] {
            assert!(local_paths(uri.as_bytes()).is_none(), "{uri}");
        }
    }
    #[test]
    fn bounds_payload_and_file_count() {
        assert!(local_paths(&vec![b'x'; MAX_BYTES + 1]).is_none());
        let bytes = (0..=MAX_FILES)
            .map(|i| format!("file:///tmp/{i}.pdf\r\n"))
            .collect::<String>();
        assert!(local_paths(bytes.as_bytes()).is_none());
    }
}
