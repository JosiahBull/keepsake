//! What is served: the file written for each kind of upload.
mod support;
use support as db;
#[path = "support/media_files.rs"]
mod files;

use image::ImageFormat;
use keepsake_core::media::{self, MediaUpload, inspect};

async fn stored(declared: &str, bytes: Vec<u8>) -> (MediaUpload, Vec<u8>) {
    let directory = tempfile::tempdir().unwrap();
    let url = format!("sqlite:{}", directory.path().join("render.db").display());
    let pool = db::connect(&url).await.unwrap();
    let upload = media::upload(&pool, 1, declared, bytes)
        .await
        .unwrap_or_else(|error| panic!("{declared}: {error}"));
    let path = media::locate(&pool, 1, upload.media_id)
        .await
        .unwrap()
        .unwrap()
        .path;
    let data = std::fs::read(path).unwrap();
    pool.close().await;
    (upload, data)
}

/// The offset of the first top-level or nested box of this type.
fn find(data: &[u8], kind: &[u8; 4]) -> Option<usize> {
    data.windows(4).position(|window| window == kind)
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// An AVIF names its brand in `ftyp` and its size in `ispe`.
fn avif_size(data: &[u8]) -> (u32, u32) {
    assert_eq!(&data[4..12], b"ftypavif", "not an AVIF");
    let ispe = find(data, b"ispe").expect("AVIF has no image size");
    (u32_at(data, ispe + 8), u32_at(data, ispe + 12))
}

#[tokio::test]
async fn still_images_become_upright_avif_no_larger_than_2048_pixels() {
    for (declared, bytes, expected) in [
        ("image/png", files::png(1), (4, 3)),
        (
            "image/webp",
            files::image(ImageFormat::WebP, 30, 20, 1),
            (30, 20),
        ),
        ("image/png", files::translucent_png(), (8, 8)),
        (
            "image/png",
            files::image(ImageFormat::Png, 3000, 1000, 1),
            (2048, 683),
        ),
        // Stored 40 wide; EXIF says to turn it, so it is served 20 wide.
        ("image/jpeg", files::sideways_jpeg(40, 20), (20, 40)),
    ] {
        let (upload, data) = stored(declared, bytes).await;
        assert_eq!(upload.content_type, "image/avif", "{declared}");
        assert_eq!(avif_size(&data), expected, "{declared}");
        assert_eq!(
            (upload.width, upload.height),
            (Some(expected.0.into()), Some(expected.1.into()))
        );
        assert_eq!(upload.byte_size, data.len() as i64);
    }
}

#[tokio::test]
async fn every_clip_becomes_aac_in_an_mp4_with_its_index_first() {
    for (declared, bytes, expected_ms) in [
        ("audio/wav", files::wav(250, 3), 250),
        ("audio/wav", files::tone(44_100, 2, 400), 400),
        ("audio/wav", files::tone(8_000, 1, 300), 300),
        ("audio/mpeg", files::mp3(20), 522),
        ("audio/mp4", files::M4A.to_vec(), 500),
        ("audio/aac", files::AAC.to_vec(), 500),
        ("audio/flac", files::FLAC.to_vec(), 500),
        ("audio/ogg", files::ogg_opus(10), 200),
        ("audio/webm", files::webm_opus(10), 200),
    ] {
        let (upload, data) = stored(declared, bytes).await;
        assert_eq!(upload.content_type, "audio/mp4", "{declared}");
        assert_eq!(&data[4..12], b"ftypM4A ", "{declared}");
        // The index arrives before the audio, so playback and seeking start at once.
        assert!(
            find(&data, b"moov").unwrap() < find(&data, b"mdat").unwrap(),
            "{declared}"
        );
        let measured = upload.duration_ms.unwrap();
        assert!(
            (measured - expected_ms).abs() <= 140,
            "{declared}: {measured} ms, expected about {expected_ms}"
        );
        // Players trim the encoder's priming and padding by the edit list, which must
        // state the source's length and skip at least the priming.
        let elst = find(&data, b"elst").unwrap();
        assert_eq!(i64::from(u32_at(&data, elst + 12)), measured, "{declared}");
        assert!(
            u32_at(&data, elst + 16) > 0,
            "{declared}: no priming skipped"
        );
        // Our own decoder reads it back whole. It ignores the edit list, so it also counts
        // the priming and the padded last frame: at most three AAC frames more.
        let mdhd = find(&data, b"mdhd").unwrap();
        let rate = i64::from(u32_at(&data, mdhd + 16));
        let reread = inspect("audio/mp4", &data)
            .unwrap_or_else(|error| panic!("{declared} rendition: {error}"))
            .duration_ms
            .unwrap();
        assert!(
            (0..=3 * 1024 * 1000 / rate + 1).contains(&(reread - measured)),
            "{declared}: rendition decodes to {reread} ms, source {measured} ms"
        );
    }
}

#[tokio::test]
async fn sample_rates_above_48_khz_are_halved() {
    let (upload, data) = stored("audio/wav", files::tone(96_000, 1, 300)).await;
    assert_eq!(upload.duration_ms, Some(300));
    let mdhd = find(&data, b"mdhd").unwrap();
    // Version and flags, then creation and modification times, then the timescale.
    assert_eq!(u32_at(&data, mdhd + 16), 48_000);
}
