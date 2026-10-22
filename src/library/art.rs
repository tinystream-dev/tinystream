// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{ImageReader, Limits};

pub const ARTWORK_MAX: usize = 8 * 1024 * 1024;
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif"];
pub const POSTER_NAMES: &[&str] = &["poster", "thumbnail", "folder", "cover"];
pub const BACKDROP_NAMES: &[&str] = &["backdrop", "banner", "fanart", "background"];

pub fn local_art(dir: &Path, names: &[&str]) -> Option<PathBuf> {
    for name in names {
        for ext in IMAGE_EXTENSIONS {
            let p = dir.join(format!("{name}.{ext}"));

            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn local_still(video: &Path) -> Option<PathBuf> {
    let stem = video.file_stem()?.to_str()?;
    local_art(video.parent()?, &[&format!("{stem}.thumbnail"), stem])
}

pub fn local_version(path: Option<&Path>) -> String {
    path.and_then(|p| p.metadata().ok())
        .and_then(|m| {
            let time = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
            Some(format!("{}-{}", time.as_nanos(), m.len()))
        })
        .unwrap_or_else(|| "0".into())
}

pub fn image_type(data: &[u8]) -> Result<&'static str, &'static str> {
    if data.len() > ARTWORK_MAX {
        return Err("that picture is too big (8 MB at most)");
    }
    match data {
        [0x89, b'P', b'N', b'G', 13, 10, 26, 10, ..] => Ok("image/png"),
        [0xFF, 0xD8, 0xFF, ..] => Ok("image/jpeg"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Ok("image/webp"),
        [b'G', b'I', b'F', b'8', b'7' | b'9', b'a', ..] => Ok("image/gif"),
        _ => Err("pictures have to be PNG, JPEG, WebP or GIF"),
    }
}

pub fn validate_image(data: &[u8]) -> Result<(), &'static str> {
    image_type(data)?;

    let mut reader =
        ImageReader::new(Cursor::new(data)).with_guessed_format().map_err(|_| "that picture could not be decoded")?;

    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|_| "that picture is corrupt or exceeds the image limits")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn directory() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);

        let p = std::env::temp_dir().join(format!(
            "tinystream-art-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn title_aliases_and_existing_precedence() {
        let dir = directory();
        fs::write(dir.join("thumbnail.png"), []).unwrap();
        fs::write(dir.join("banner.webp"), []).unwrap();
        assert_eq!(local_art(&dir, POSTER_NAMES), Some(dir.join("thumbnail.png")));
        assert_eq!(local_art(&dir, BACKDROP_NAMES), Some(dir.join("banner.webp")));
        fs::write(dir.join("poster.jpg"), []).unwrap();
        assert_eq!(local_art(&dir, POSTER_NAMES), Some(dir.join("poster.jpg")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn episode_sidecars_are_specific_to_the_video() {
        let dir = directory();
        fs::write(dir.join("Episode 1.png"), []).unwrap();
        fs::write(dir.join("Episode 1.thumbnail.webp"), []).unwrap();
        let video = dir.join("Episode 1.mkv");
        assert_eq!(local_still(&video), Some(dir.join("Episode 1.thumbnail.webp")));
        assert_eq!(local_still(&dir.join("Episode 2.mkv")), None);
        fs::remove_file(dir.join("Episode 1.thumbnail.webp")).unwrap();
        assert_eq!(local_still(&video), Some(dir.join("Episode 1.png")));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn old_jpeg_sidecars_and_all_image_formats_work() {
        let dir = directory();

        for ext in IMAGE_EXTENSIONS {
            let image = dir.join(format!("Episode.{ext}"));
            fs::write(&image, []).unwrap();
            assert_eq!(local_still(&dir.join("Episode.mp4")), Some(image.clone()));
            fs::remove_file(image).unwrap();
        }

        fs::create_dir(dir.join("thumbnail.png")).unwrap();
        fs::write(dir.join("thumbnail.svg"), []).unwrap();
        assert_eq!(local_art(&dir, POSTER_NAMES), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn replacing_a_sidecar_changes_its_cache_version() {
        let dir = directory();
        let image = dir.join("thumbnail.png");
        fs::write(&image, b"first").unwrap();
        let before = local_version(Some(&image));
        fs::write(&image, b"second picture").unwrap();
        assert_ne!(local_version(Some(&image)), before);
        assert_eq!(local_version(None), "0");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn uploads_use_content_type_and_reject_unsupported_or_oversized_data() {
        assert_eq!(image_type(b"\x89PNG\r\n\x1a\n"), Ok("image/png"));
        assert_eq!(image_type(b"\xff\xd8\xff"), Ok("image/jpeg"));
        assert_eq!(image_type(b"RIFF0000WEBP"), Ok("image/webp"));
        assert_eq!(image_type(b"GIF89a"), Ok("image/gif"));
        assert!(image_type(b"<svg></svg>").is_err());
        assert!(image_type(b"\x89PNG").is_err());
        assert!(image_type(&vec![0; ARTWORK_MAX + 1]).is_err());
    }

    #[test]
    fn uploads_require_decodable_images() {
        for header in [b"\x89PNG\r\n\x1a\n".as_slice(), b"\xff\xd8\xff", b"RIFF0000WEBP", b"GIF89a"] {
            assert!(validate_image(header).is_err());
        }

        let picture = image::DynamicImage::new_rgb8(2, 2);

        for format in
            [image::ImageFormat::Png, image::ImageFormat::Jpeg, image::ImageFormat::WebP, image::ImageFormat::Gif]
        {
            let mut data = Cursor::new(Vec::new());
            picture.write_to(&mut data, format).unwrap();
            assert_eq!(validate_image(data.get_ref()), Ok(()));
            let half = data.get_ref().len() / 2;
            data.get_mut().truncate(half);
            assert!(validate_image(data.get_ref()).is_err());
        }
    }

    #[test]
    fn uploads_limit_decoded_dimensions() {
        let picture = image::DynamicImage::new_luma8(8193, 1);
        let mut data = Cursor::new(Vec::new());
        picture.write_to(&mut data, image::ImageFormat::Png).unwrap();
        assert!(data.get_ref().len() < ARTWORK_MAX);
        assert!(validate_image(data.get_ref()).is_err());
    }
}
