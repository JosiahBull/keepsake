//! What may be stored: a declared, supported type whose bytes decode as exactly that.
//!
//! Three checks, each of which alone would be easy to fool. The declared type must be on
//! the allowlist. The file's signature must name the same format. Then the whole file is
//! decoded by a pure-Rust decoder for that format, within limits on dimensions, frames,
//! duration and memory. A renamed file, a truncated one, or something else wearing a
//! valid header fails here, before anything reaches the database.
use std::io::Cursor;

use image::{AnimationDecoder, ImageDecoder, ImageFormat};
use symphonia::core::{
    codecs::{CodecParameters, audio::AudioDecoderOptions, audio::well_known::CODEC_ID_OPUS},
    errors::Error as AudioError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::Timestamp,
};

use super::{MAX_MEDIA_BYTES, MediaKind, invalid};

/// Images larger than this on either side are refused rather than scaled.
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// Clips longer than this are refused. Measured from the stream, not its header.
pub const MAX_AUDIO_MS: i64 = 5 * 60 * 1000;
/// Frames decoded from one animated GIF or WebP before it is refused.
const MAX_FRAMES: usize = 200;
/// Decoder working memory per image. A 4096 × 4096 RGBA frame is 64 MiB.
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
/// Decode errors tolerated in a clip whose other packets decode, as some encoders
/// emit a damaged first frame. More than this is a damaged file.
const TOLERATED_AUDIO_ERRORS: usize = 2;

/// What an unsupported upload is told it can use instead.
pub const SUPPORTED_FORMATS: &str =
    "Use a JPEG, PNG, GIF or WebP image, or an MP3, M4A, AAC, WAV, Ogg, WebM or FLAC clip";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decoder {
    Image(ImageFormat),
    /// The container name Symphonia reports for this format.
    Audio(&'static str),
}

struct Format {
    content_type: &'static str,
    /// Reads after "a" or "an", whichever `article` says.
    label: &'static str,
    article: &'static str,
    decoder: Decoder,
}

const FORMATS: [Format; 11] = [
    Format {
        content_type: "image/jpeg",
        label: "JPEG image",
        article: "a",
        decoder: Decoder::Image(ImageFormat::Jpeg),
    },
    Format {
        content_type: "image/png",
        label: "PNG image",
        article: "a",
        decoder: Decoder::Image(ImageFormat::Png),
    },
    Format {
        content_type: "image/gif",
        label: "GIF image",
        article: "a",
        decoder: Decoder::Image(ImageFormat::Gif),
    },
    Format {
        content_type: "image/webp",
        label: "WebP image",
        article: "a",
        decoder: Decoder::Image(ImageFormat::WebP),
    },
    Format {
        content_type: "audio/mpeg",
        label: "MP3 clip",
        article: "an",
        decoder: Decoder::Audio("mp3"),
    },
    Format {
        content_type: "audio/aac",
        label: "AAC clip",
        article: "an",
        decoder: Decoder::Audio("aac"),
    },
    Format {
        content_type: "audio/mp4",
        label: "M4A clip",
        article: "an",
        decoder: Decoder::Audio("isomp4"),
    },
    Format {
        content_type: "audio/wav",
        label: "WAV clip",
        article: "a",
        decoder: Decoder::Audio("wave"),
    },
    Format {
        content_type: "audio/ogg",
        label: "Ogg clip",
        article: "an",
        decoder: Decoder::Audio("ogg"),
    },
    Format {
        content_type: "audio/webm",
        label: "WebM clip",
        article: "a",
        decoder: Decoder::Audio("matroska"),
    },
    Format {
        content_type: "audio/flac",
        label: "FLAC clip",
        article: "a",
        decoder: Decoder::Audio("flac"),
    },
];

/// A file that passed every check, with what was measured while decoding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub kind: MediaKind,
    pub content_type: &'static str,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub duration_ms: Option<i64>,
}

/// The canonical content type for a declared one, accepting the aliases browsers and
/// operating systems actually send. Parameters such as `;codecs=opus` are ignored.
pub fn canonical_type(declared: &str) -> Option<&'static str> {
    let essence = declared
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    Some(match essence.as_str() {
        "image/jpeg" | "image/jpg" | "image/pjpeg" => "image/jpeg",
        "image/png" => "image/png",
        "image/gif" => "image/gif",
        "image/webp" => "image/webp",
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" | "audio/x-mp3" => "audio/mpeg",
        "audio/aac" | "audio/x-aac" | "audio/aacp" => "audio/aac",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "audio/mp4",
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => "audio/wav",
        "audio/ogg" | "audio/opus" | "audio/vorbis" | "application/ogg" => "audio/ogg",
        // Browsers label any .webm file as video; the decoder refuses one that holds video.
        "audio/webm" | "video/webm" => "audio/webm",
        "audio/flac" | "audio/x-flac" => "audio/flac",
        _ => return None,
    })
}

/// The declared type for a file named with this extension, for callers without one.
pub fn type_for_extension(extension: &str) -> Option<&'static str> {
    Some(match extension.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "mp3" => "audio/mpeg",
        "aac" => "audio/aac",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "webm" => "audio/webm",
        "flac" => "audio/flac",
        _ => return None,
    })
}

/// The format a file's signature names, if it is one Keepsake stores.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let at = |offset: usize, signature: &[u8]| {
        bytes.get(offset..offset + signature.len()) == Some(signature)
    };
    if at(0, b"\xFF\xD8\xFF") {
        return Some("image/jpeg");
    }
    if at(0, b"\x89PNG\r\n\x1A\n") {
        return Some("image/png");
    }
    if at(0, b"GIF87a") || at(0, b"GIF89a") {
        return Some("image/gif");
    }
    if at(0, b"RIFF") && at(8, b"WEBP") {
        return Some("image/webp");
    }
    if at(0, b"RIFF") && at(8, b"WAVE") {
        return Some("audio/wav");
    }
    if at(0, b"OggS") {
        return Some("audio/ogg");
    }
    if at(0, b"fLaC") {
        return Some("audio/flac");
    }
    if at(0, b"ID3") {
        return Some("audio/mpeg");
    }
    if at(4, b"ftyp") {
        // ISO media brands for audio. Video brands and HEIC stills are refused.
        return matches!(bytes.get(8..12)?, b"M4A " | b"M4B " | b"mp42" | b"isom")
            .then_some("audio/mp4");
    }
    if at(0, b"\x1A\x45\xDF\xA3") {
        // EBML names its document type near the start; other Matroska is refused.
        let header = &bytes[..bytes.len().min(64)];
        return header
            .windows(4)
            .any(|window| window == b"webm")
            .then_some("audio/webm");
    }
    match bytes {
        // ADTS AAC: a frame sync with MPEG layer bits of zero.
        [0xFF, second, ..] if second & 0xF6 == 0xF0 => Some("audio/aac"),
        // MPEG audio Layer III frame sync, as in an MP3 without ID3 tags.
        [0xFF, second, ..] if second & 0xE0 == 0xE0 && (second >> 1) & 0b11 == 0b01 => {
            Some("audio/mpeg")
        }
        _ => None,
    }
}

/// Decide whether `bytes` may be stored as `declared`. CPU-bound: call off the async runtime.
pub fn inspect(declared: &str, bytes: &[u8]) -> anyhow::Result<Inspection> {
    if bytes.is_empty() {
        return Err(invalid("The file is empty"));
    }
    if bytes.len() > MAX_MEDIA_BYTES {
        return Err(invalid("Files must be 5 MiB or smaller"));
    }
    let format = canonical_type(declared)
        .and_then(|content_type| FORMATS.iter().find(|f| f.content_type == content_type))
        .ok_or_else(|| {
            // The declared type is caller text: bound and quote it.
            invalid(format!(
                "Unsupported file type {:?}. {SUPPORTED_FORMATS}.",
                declared.chars().take(100).collect::<String>()
            ))
        })?;
    if sniff(bytes) != Some(format.content_type) {
        return Err(invalid(format!(
            "The file is not {} {}. Its contents do not match its type.",
            format.article, format.label
        )));
    }
    match format.decoder {
        Decoder::Image(image_format) => {
            let (width, height) = inspect_image(image_format, format.label, bytes)?;
            Ok(Inspection {
                kind: MediaKind::Image,
                content_type: format.content_type,
                width: Some(width.into()),
                height: Some(height.into()),
                duration_ms: None,
            })
        }
        Decoder::Audio(container) => Ok(Inspection {
            kind: MediaKind::Audio,
            content_type: format.content_type,
            width: None,
            height: None,
            duration_ms: inspect_audio(container, format, bytes)?,
        }),
    }
}

fn image_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

fn image_error(label: &str) -> impl Fn(image::ImageError) -> anyhow::Error + '_ {
    move |error| match error {
        image::ImageError::Limits(_) => invalid(format!(
            "Resize the image to at most {MAX_IMAGE_SIDE} pixels on each side."
        )),
        _ => invalid(format!("The {label} is damaged or incomplete.")),
    }
}

fn inspect_image(format: ImageFormat, label: &str, bytes: &[u8]) -> anyhow::Result<(u32, u32)> {
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(image_limits());
    let image = reader.decode().map_err(image_error(label))?;
    let dimensions = (image.width(), image.height());
    drop(image);
    // A good first frame must not vouch for the rest of an animation.
    match format {
        ImageFormat::Gif => frames(
            image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(image_error(label))?,
            label,
        )?,
        ImageFormat::WebP => {
            let decoder = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
                .map_err(image_error(label))?;
            if decoder.has_animation() {
                frames(decoder, label)?;
            }
        }
        _ => {}
    }
    Ok(dimensions)
}

fn frames<'a, D: AnimationDecoder<'a> + ImageDecoder>(
    mut decoder: D,
    label: &str,
) -> anyhow::Result<()> {
    decoder
        .set_limits(image_limits())
        .map_err(image_error(label))?;
    for (index, frame) in decoder.into_frames().enumerate() {
        if index >= MAX_FRAMES {
            return Err(invalid(format!(
                "Animations must have at most {MAX_FRAMES} frames."
            )));
        }
        frame.map_err(image_error(label))?;
    }
    Ok(())
}

/// Demux every packet of the clip's audio track and decode each one. Returns the
/// measured duration, or `None` when the stream does not say how long its packets are.
fn inspect_audio(container: &str, format: &Format, bytes: &[u8]) -> anyhow::Result<Option<i64>> {
    let label = format.label;
    let damaged = || invalid(format!("The {label} is damaged or incomplete."));
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut reader = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| damaged())?;
    if reader.format_info().short_name != container {
        return Err(invalid(format!(
            "The file is not {} {label}. Its contents do not match its type.",
            format.article
        )));
    }
    if reader
        .tracks()
        .iter()
        .any(|track| matches!(track.codec_params, Some(CodecParameters::Video(_))))
    {
        return Err(invalid(
            "Video files are not supported. Choose an audio clip.",
        ));
    }
    let track = reader.default_track(TrackType::Audio).ok_or_else(damaged)?;
    let track_id = track.id;
    let time_base = track.time_base;
    let declared_frames = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(CodecParameters::audio)
        .ok_or_else(damaged)?
        .clone();
    if params
        .sample_rate
        .is_some_and(|rate| !(8_000..=192_000).contains(&rate))
        || params
            .channels
            .as_ref()
            .is_some_and(|channels| channels.count() > 8)
    {
        return Err(damaged());
    }
    let mut decoder = match symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
    {
        Ok(decoder) => Some(decoder),
        // No pure-Rust Opus decoder exists here; its container is still fully demuxed.
        Err(_) if params.codec == CODEC_ID_OPUS && matches!(container, "ogg" | "matroska") => None,
        Err(_) => {
            return Err(invalid(format!(
                "The {label} uses an audio codec Keepsake cannot check."
            )));
        }
    };

    let milliseconds = |units: u64| {
        time_base
            .and_then(|base| base.calc_time(Timestamp::new(i64::try_from(units).ok()?)))
            .map(|time| time.as_millis())
    };
    let (mut packets, mut decoded, mut errors, mut units) = (0usize, 0usize, 0usize, 0u64);
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(_) => return Err(damaged()),
        };
        if packet.track_id != track_id {
            continue;
        }
        packets += 1;
        units = units.saturating_add(packet.dur.get());
        // Stop before decoding anything past the limit.
        if milliseconds(units).is_some_and(|ms| ms > i128::from(MAX_AUDIO_MS)) {
            return Err(invalid(format!(
                "Audio clips must be {} minutes or shorter.",
                MAX_AUDIO_MS / 60_000
            )));
        }
        if let Some(decoder) = decoder.as_mut() {
            match decoder.decode(&packet) {
                Ok(buffer) if buffer.frames() > 0 => decoded += 1,
                Ok(_) => {}
                Err(AudioError::DecodeError(_)) => errors += 1,
                Err(_) => return Err(damaged()),
            }
        }
    }
    if packets == 0 || (decoder.is_some() && (decoded == 0 || errors > TOLERATED_AUDIO_ERRORS)) {
        return Err(damaged());
    }
    let measured = milliseconds(units)
        .filter(|ms| *ms > 0)
        .or_else(|| declared_frames.and_then(milliseconds));
    if measured.is_some_and(|ms| ms > i128::from(MAX_AUDIO_MS)) {
        return Err(invalid(format!(
            "Audio clips must be {} minutes or shorter.",
            MAX_AUDIO_MS / 60_000
        )));
    }
    Ok(measured.map(|ms| ms as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_name_only_the_formats_that_are_stored() {
        for (bytes, expected) in [
            (b"\xFF\xD8\xFF\xE0".as_slice(), Some("image/jpeg")),
            (b"\x89PNG\r\n\x1A\n", Some("image/png")),
            (b"GIF89a", Some("image/gif")),
            (b"RIFF\0\0\0\0WEBPVP8 ", Some("image/webp")),
            (b"RIFF\0\0\0\0WAVEfmt ", Some("audio/wav")),
            (b"OggS\0", Some("audio/ogg")),
            (b"fLaC", Some("audio/flac")),
            (b"ID3\x04", Some("audio/mpeg")),
            (b"\xFF\xFB\x90\x00", Some("audio/mpeg")),
            (b"\xFF\xF1\x50\x80", Some("audio/aac")),
            (b"\0\0\0\x20ftypM4A ", Some("audio/mp4")),
            (
                b"\x1A\x45\xDF\xA3\x9F\x42\x86\x81\x01\x42\x82\x84webm",
                Some("audio/webm"),
            ),
            (b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>", None),
            (b"<!doctype html><script>alert(1)</script>", None),
            (b"%PDF-1.7", None),
            (b"\0\0\0\x20ftypheic", None),
            (b"\0\0\0\x20ftypavif", None),
            (
                b"\x1A\x45\xDF\xA3\x9F\x42\x86\x81\x01\x42\x82\x88matroska",
                None,
            ),
            // MPEG Layer II, which browsers do not reliably play.
            (b"\xFF\xFD\x90\x00", None),
            (b"", None),
        ] {
            assert_eq!(sniff(bytes), expected, "{bytes:?}");
        }
    }

    #[test]
    fn declared_types_accept_real_aliases_and_nothing_else() {
        for (declared, expected) in [
            ("image/JPEG", Some("image/jpeg")),
            ("audio/x-m4a", Some("audio/mp4")),
            ("audio/webm;codecs=opus", Some("audio/webm")),
            ("video/webm", Some("audio/webm")),
            ("audio/x-wav", Some("audio/wav")),
            ("image/svg+xml", None),
            ("text/html", None),
            ("video/mp4", None),
            ("application/octet-stream", None),
            ("", None),
        ] {
            assert_eq!(canonical_type(declared), expected, "{declared}");
        }
    }
}
