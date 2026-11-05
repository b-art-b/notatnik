//! The pictures of an article: fetched under the same rules as the page (public addresses only, limits), recognised by
//! their bytes (never by what the server says), and never SVG.

use std::sync::Arc;

use url::Url;

use crate::net::{self, ClipPolicy, PICTURE_TIMEOUT};

pub const MAX_PICTURES: usize = 40;
pub const MAX_PICTURE_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 25 * 1024 * 1024;

pub struct Picture {
    /// `images/img3.png`, relative to the chapter.
    pub name: String,
    pub media_type: &'static str,
    pub bytes: Vec<u8>,
}

/// `(media type, extension)` of png, jpeg, gif and webp, from the first bytes; nothing else is accepted.
pub fn sniff(b: &[u8]) -> Option<(&'static str, &'static str)> {
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]) {
        Some(("image/png", "png"))
    } else if b.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else {
        None
    }
}

/// Fetches a picture; `None` when it cannot be used (any reason: a 404, too big, the wrong type, a refused address).
pub fn fetch(policy: &Arc<ClipPolicy>, url: &Url, number: usize, room: u64) -> Option<Picture> {
    let limit = MAX_PICTURE_BYTES.min(room);
    let timeout = policy.picture_timeout.unwrap_or(PICTURE_TIMEOUT);
    let got = net::get(
        policy,
        url,
        "image/png,image/jpeg,image/gif,image/webp,*/*;q=0.1",
        limit,
        timeout,
    )
    .ok()?;
    let (media_type, ext) = sniff(&got.body)?;
    Some(Picture {
        name: format!("images/img{number}.{ext}"),
        media_type,
        bytes: got.body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_are_known_by_their_bytes() {
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0]),
            Some(("image/png", "png"))
        );
        assert_eq!(
            sniff(&[0xff, 0xd8, 0xff, 0xe0]),
            Some(("image/jpeg", "jpg"))
        );
        assert_eq!(sniff(b"GIF89a...."), Some(("image/gif", "gif")));
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some(("image/webp", "webp")));
        assert_eq!(
            sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            None
        );
        assert_eq!(sniff(b"<html>"), None);
        assert_eq!(sniff(b""), None);
    }
}
