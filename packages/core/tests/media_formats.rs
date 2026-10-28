//! What the upload inspection accepts and refuses, using real encoded files.
#[path = "support/media_files.rs"]
mod files;

use image::ImageFormat;
use keepsake_core::media::{Inspection, MediaKind, UploadFormat, inspect};

/// A declared MIME type parsed as callers parse it, then the upload inspected.
fn inspected(declared: &str, bytes: &[u8]) -> anyhow::Result<Inspection> {
    inspect(UploadFormat::parse_content_type(declared)?, bytes)
}

fn accepted(declared: &str, bytes: &[u8]) -> Inspection {
    inspected(declared, bytes).unwrap_or_else(|error| panic!("{declared}: {error}"))
}

fn refused(declared: &str, bytes: &[u8]) -> String {
    match inspected(declared, bytes) {
        Ok(inspection) => panic!("{declared} was accepted: {inspection:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn every_supported_image_decodes_with_its_dimensions() {
    for (declared, format) in [
        ("image/png", ImageFormat::Png),
        ("image/jpeg", ImageFormat::Jpeg),
        ("image/webp", ImageFormat::WebP),
    ] {
        let inspection = accepted(declared, &files::image(format, 40, 30, 7));
        assert_eq!(inspection.format.kind(), MediaKind::Image);
        assert_eq!(
            Some(inspection.format),
            UploadFormat::from_content_type(declared)
        );
        assert_eq!((inspection.width, inspection.height), (Some(40), Some(30)));
        assert_eq!(inspection.duration, None);
    }
}

#[test]
fn gifs_and_animations_are_refused() {
    assert!(refused("image/gif", files::GIF).starts_with("Unsupported file type"));
    assert!(refused("image/png", files::GIF).contains("Its contents do not match its type"));
    for (declared, bytes) in [
        ("image/png", files::animated_png()),
        ("image/webp", files::animated_webp()),
    ] {
        assert_eq!(
            refused(declared, &bytes),
            "Animated images are not supported. Choose a still image.",
            "{declared}"
        );
    }
}

#[test]
fn every_supported_clip_is_decoded_or_demuxed_and_timed() {
    for (declared, bytes, expected_ms) in [
        ("audio/wav", files::wav(250, 3), 250),
        ("audio/x-wav", files::wav(250, 3), 250),
        ("audio/mpeg", files::mp3(20), 522),
        ("audio/mp4", files::M4A.to_vec(), 500),
        ("audio/x-m4a", files::M4A.to_vec(), 500),
        ("audio/aac", files::AAC.to_vec(), 500),
        ("audio/flac", files::FLAC.to_vec(), 500),
        ("audio/ogg", files::ogg_opus(10), 200),
        ("audio/webm;codecs=opus", files::webm_opus(10), 200),
    ] {
        let inspection = accepted(declared, &bytes);
        assert_eq!(inspection.format.kind(), MediaKind::Audio, "{declared}");
        assert_eq!((inspection.width, inspection.height), (None, None));
        let measured = inspection
            .duration
            .unwrap_or_else(|| panic!("{declared} has no duration"))
            .as_millis() as i64;
        // Packet durations include encoder priming and padding to whole frames: AAC adds
        // 2112 samples, 132 ms at 16 kHz. Within 200 ms of the source is the same clip.
        assert!(
            (measured - expected_ms).abs() <= 200,
            "{declared}: measured {measured} ms, expected about {expected_ms}"
        );
    }
}

#[test]
fn a_file_must_be_what_it_is_declared_to_be() {
    let png = files::png(1);
    for (declared, bytes) in [
        ("image/jpeg", png.clone()),
        ("audio/mpeg", files::wav(100, 1)),
        ("audio/wav", files::mp3(4)),
        ("image/webp", files::GIF.to_vec()),
        ("audio/flac", files::ogg_opus(2)),
    ] {
        assert!(
            refused(declared, &bytes).contains("Its contents do not match its type"),
            "{declared}"
        );
    }
    for declared in [
        "image/svg+xml",
        "text/html",
        "video/mp4",
        "application/octet-stream",
        "",
    ] {
        assert!(
            refused(declared, &png).starts_with("Unsupported file type"),
            "{declared:?}"
        );
    }
}

#[test]
fn a_valid_signature_on_a_damaged_body_is_refused() {
    let png = files::png(2);
    let jpeg = files::image(ImageFormat::Jpeg, 64, 64, 2);
    let wav = files::wav(200, 2);
    for (declared, bytes) in [
        ("image/png", png[..png.len() / 2].to_vec()),
        ("image/png", [&png[..16], &[0x42; 64][..]].concat()),
        ("image/jpeg", jpeg[..jpeg.len() / 3].to_vec()),
        ("audio/wav", wav[..44].to_vec()),
        ("audio/mp4", files::M4A[..files::M4A.len() / 2].to_vec()),
        (
            "audio/flac",
            [&files::FLAC[..42], &[0x55; 512][..]].concat(),
        ),
        (
            "audio/mpeg",
            [&[0xFF, 0xFB, 0x90, 0xC0][..], &[0x99; 600][..]].concat(),
        ),
    ] {
        let message = refused(declared, &bytes);
        assert!(
            message.contains("damaged or incomplete") || message.contains("do not match"),
            "{declared}: {message}"
        );
    }
}

#[test]
fn size_dimensions_and_duration_are_bounded() {
    assert!(refused("image/png", &[]).contains("empty"));
    let oversized = [files::png(3), vec![0; 5 * 1024 * 1024]].concat();
    assert!(refused("image/png", &oversized).contains("5 MiB"));
    assert!(
        refused("image/png", &files::image(ImageFormat::Png, 4097, 1, 3)).contains("4096 pixels")
    );
    accepted("image/png", &files::image(ImageFormat::Png, 4096, 1, 3));

    let long = files::long_wav(301_000);
    assert!(long.len() < 5 * 1024 * 1024);
    assert!(refused("audio/wav", &long).contains("5 minutes"));
    accepted("audio/wav", &files::long_wav(299_000));
}

#[test]
fn a_video_is_refused_even_when_it_carries_audio() {
    assert!(
        refused("video/webm", &files::webm_with_video(5)).contains("Video files are not supported")
    );
}
