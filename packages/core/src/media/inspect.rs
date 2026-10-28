//! Accepting an upload: a declared format, a matching signature, and a full decode.
//!
//! Only what it takes to decode each accepted format lives here. Nothing is ever served in
//! these formats: the renderer turns what is decoded into the one format each kind of
//! media is served in. A renamed file, a truncated one, or something else wearing a valid
//! header fails here, before anything is written.
use std::{io::Cursor, sync::LazyLock, time::Duration};

use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat};
use strum::EnumIter;
use symphonia::core::{
    codecs::{CodecParameters, audio::AudioDecoderOptions, registry::CodecRegistry},
    errors::Error as AudioError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::{TimeBase, Timestamp},
};
use symphonia_adapter_libopus::OpusDecoder;

use super::{MAX_MEDIA_BYTES, MediaKind, invalid};

/// Images larger than this on either side are refused rather than scaled.
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// Clips longer than this are refused. Measured from the stream, not its header.
pub const MAX_AUDIO: Duration = Duration::from_secs(5 * 60);
/// Decoder working memory per image. A 4096 × 4096 RGBA frame is 64 MiB.
const MAX_DECODE_BYTES: u64 = 128 * 1024 * 1024;
/// Decode errors tolerated in a clip whose other packets decode, as some encoders
/// emit a damaged first frame. More than this is a damaged file.
const TOLERATED_AUDIO_ERRORS: usize = 2;

/// What an unsupported upload is told it can use instead. A test checks it names every
/// [`UploadFormat`].
pub const SUPPORTED_FORMATS: &str =
    "Use a JPEG, PNG or WebP image, or an MP3, M4A, AAC, WAV, Ogg, WebM or FLAC clip";

/// A format Keepsake accepts and decodes. None of them is served.
#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter)]
pub enum UploadFormat {
    Jpeg,
    Png,
    WebP,
    Mp3,
    Aac,
    M4a,
    Wav,
    Ogg,
    WebM,
    Flac,
}

/// How an accepted format is decoded.
enum Decoder {
    Image(ImageCodec),
    Audio(Container),
}

/// The still-image codecs accepted. GIF is not one of them.
#[derive(Clone, Copy)]
enum ImageCodec {
    Jpeg,
    Png,
    WebP,
}

impl ImageCodec {
    const fn format(self) -> ImageFormat {
        match self {
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Png => ImageFormat::Png,
            Self::WebP => ImageFormat::WebP,
        }
    }
}

/// The audio containers Symphonia demuxes for accepted formats.
#[derive(Clone, Copy)]
enum Container {
    Mp3,
    Adts,
    IsoMp4,
    Wave,
    Ogg,
    Matroska,
    Flac,
}

impl Container {
    /// The name Symphonia's probe reports when it recognises this container.
    const fn probed_as(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Adts => "aac",
            Self::IsoMp4 => "isomp4",
            Self::Wave => "wave",
            Self::Ogg => "ogg",
            Self::Matroska => "matroska",
            Self::Flac => "flac",
        }
    }
}

impl UploadFormat {
    pub const fn kind(self) -> MediaKind {
        match self {
            Self::Jpeg | Self::Png | Self::WebP => MediaKind::Image,
            Self::Mp3 | Self::Aac | Self::M4a | Self::Wav | Self::Ogg | Self::WebM | Self::Flac => {
                MediaKind::Audio
            }
        }
    }

    /// The format's name in messages, read after [`UploadFormat::article`].
    pub const fn name(self) -> &'static str {
        match self {
            Self::Jpeg => "JPEG image",
            Self::Png => "PNG image",
            Self::WebP => "WebP image",
            Self::Mp3 => "MP3 clip",
            Self::Aac => "AAC clip",
            Self::M4a => "M4A clip",
            Self::Wav => "WAV clip",
            Self::Ogg => "Ogg clip",
            Self::WebM => "WebM clip",
            Self::Flac => "FLAC clip",
        }
    }

    const fn article(self) -> &'static str {
        match self {
            Self::Mp3 | Self::Aac | Self::M4a | Self::Ogg => "an",
            Self::Jpeg | Self::Png | Self::WebP | Self::Wav | Self::WebM | Self::Flac => "a",
        }
    }

    const fn decoder(self) -> Decoder {
        match self {
            Self::Jpeg => Decoder::Image(ImageCodec::Jpeg),
            Self::Png => Decoder::Image(ImageCodec::Png),
            Self::WebP => Decoder::Image(ImageCodec::WebP),
            Self::Mp3 => Decoder::Audio(Container::Mp3),
            Self::Aac => Decoder::Audio(Container::Adts),
            Self::M4a => Decoder::Audio(Container::IsoMp4),
            Self::Wav => Decoder::Audio(Container::Wave),
            Self::Ogg => Decoder::Audio(Container::Ogg),
            Self::WebM => Decoder::Audio(Container::Matroska),
            Self::Flac => Decoder::Audio(Container::Flac),
        }
    }

    /// The format a declared MIME type names, accepting the aliases browsers and operating
    /// systems send. Parameters such as `;codecs=opus` are ignored.
    pub fn from_content_type(declared: &str) -> Option<Self> {
        let essence = declared
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        Some(match essence.as_str() {
            "image/jpeg" | "image/jpg" | "image/pjpeg" => Self::Jpeg,
            "image/png" => Self::Png,
            "image/webp" => Self::WebP,
            "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" | "audio/x-mp3" => {
                Self::Mp3
            }
            "audio/aac" | "audio/x-aac" | "audio/aacp" => Self::Aac,
            "audio/mp4" | "audio/m4a" | "audio/x-m4a" => Self::M4a,
            "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => Self::Wav,
            "audio/ogg" | "audio/opus" | "audio/vorbis" | "application/ogg" => Self::Ogg,
            // Browsers label any .webm file as video; decoding refuses one that holds video.
            "audio/webm" | "video/webm" => Self::WebM,
            "audio/flac" | "audio/x-flac" => Self::Flac,
            _ => return None,
        })
    }

    /// [`UploadFormat::from_content_type`], refusing anything else with what to use instead.
    pub fn parse_content_type(declared: &str) -> anyhow::Result<Self> {
        Self::from_content_type(declared).ok_or_else(|| {
            // The declared type is caller text: bound and quote it.
            invalid(format!(
                "Unsupported file type {:?}. {SUPPORTED_FORMATS}.",
                declared.chars().take(100).collect::<String>()
            ))
        })
    }

    /// The format a file named with this extension claims to be, for callers without a
    /// declared type.
    pub fn from_extension(extension: &str) -> Option<Self> {
        Some(match extension.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Self::Jpeg,
            "png" => Self::Png,
            "webp" => Self::WebP,
            "mp3" => Self::Mp3,
            "aac" => Self::Aac,
            "m4a" => Self::M4a,
            "wav" => Self::Wav,
            "ogg" | "oga" | "opus" => Self::Ogg,
            "webm" => Self::WebM,
            "flac" => Self::Flac,
            _ => return None,
        })
    }

    /// The format a file's signature names, if it is one Keepsake accepts.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        let at = |offset: usize, signature: &[u8]| {
            bytes.get(offset..offset + signature.len()) == Some(signature)
        };
        if at(0, b"\xFF\xD8\xFF") {
            return Some(Self::Jpeg);
        }
        if at(0, b"\x89PNG\r\n\x1A\n") {
            return Some(Self::Png);
        }
        if at(0, b"RIFF") && at(8, b"WEBP") {
            return Some(Self::WebP);
        }
        if at(0, b"RIFF") && at(8, b"WAVE") {
            return Some(Self::Wav);
        }
        if at(0, b"OggS") {
            return Some(Self::Ogg);
        }
        if at(0, b"fLaC") {
            return Some(Self::Flac);
        }
        if at(0, b"ID3") {
            return Some(Self::Mp3);
        }
        if at(4, b"ftyp") {
            // ISO media brands for audio. Video brands and HEIC stills are refused.
            return matches!(bytes.get(8..12)?, b"M4A " | b"M4B " | b"mp42" | b"isom")
                .then_some(Self::M4a);
        }
        if at(0, b"\x1A\x45\xDF\xA3") {
            // EBML names its document type near the start; other Matroska is refused.
            let header = &bytes[..bytes.len().min(64)];
            return header
                .windows(4)
                .any(|window| window == b"webm")
                .then_some(Self::WebM);
        }
        match bytes {
            // ADTS AAC: a frame sync with MPEG layer bits of zero.
            [0xFF, second, ..] if second & 0xF6 == 0xF0 => Some(Self::Aac),
            // MPEG audio Layer III frame sync, as in an MP3 without ID3 tags.
            [0xFF, second, ..] if second & 0xE0 == 0xE0 && (second >> 1) & 0b11 == 0b01 => {
                Some(Self::Mp3)
            }
            _ => None,
        }
    }
}

/// An upload that passed every check, with what was measured while decoding it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inspection {
    pub format: UploadFormat,
    /// Images only, after turning the way EXIF says.
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Clips only.
    pub duration: Option<Duration>,
}

/// What decoding produced, for the renderer: never stored as it is.
pub(crate) enum Content {
    /// A single image, already turned the way its EXIF orientation says.
    Still(DynamicImage),
    Audio(Pcm),
}

/// The channels kept from a clip. Anything beyond the first two is dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Layout {
    Mono,
    Stereo,
}

impl Layout {
    pub const fn channels(self) -> usize {
        match self {
            Self::Mono => 1,
            Self::Stereo => 2,
        }
    }
}

/// Interleaved 16-bit samples, at most 48 kHz.
pub(crate) struct Pcm {
    pub rate: u32,
    pub layout: Layout,
    pub samples: Vec<i16>,
}

impl Pcm {
    pub fn frames(&self) -> u64 {
        (self.samples.len() / self.layout.channels()) as u64
    }
}

/// Size and signature only: enough to refuse a file before hashing or decoding it.
pub(crate) fn precheck(format: UploadFormat, bytes: &[u8]) -> anyhow::Result<()> {
    if bytes.is_empty() {
        return Err(invalid("The file is empty"));
    }
    if bytes.len() > MAX_MEDIA_BYTES {
        return Err(invalid("Files must be 5 MiB or smaller"));
    }
    if UploadFormat::sniff(bytes) != Some(format) {
        return Err(not_the_format(format));
    }
    Ok(())
}

fn not_the_format(format: UploadFormat) -> anyhow::Error {
    invalid(format!(
        "The file is not {} {}. Its contents do not match its type.",
        format.article(),
        format.name()
    ))
}

fn damaged(format: UploadFormat) -> anyhow::Error {
    invalid(format!("The {} is damaged or incomplete.", format.name()))
}

/// Decide whether `bytes` may be stored as `format`. CPU-bound: call off the async runtime.
pub fn inspect(format: UploadFormat, bytes: &[u8]) -> anyhow::Result<Inspection> {
    decode(format, bytes).map(|(inspection, _)| inspection)
}

/// [`inspect`], keeping what was decoded.
pub(crate) fn decode(format: UploadFormat, bytes: &[u8]) -> anyhow::Result<(Inspection, Content)> {
    precheck(format, bytes)?;
    match format.decoder() {
        Decoder::Image(codec) => {
            let image = decode_image(format, codec, bytes)?;
            Ok((
                Inspection {
                    format,
                    width: Some(image.width()),
                    height: Some(image.height()),
                    duration: None,
                },
                Content::Still(image),
            ))
        }
        Decoder::Audio(container) => {
            let (duration, pcm) = decode_audio(format, container, bytes)?;
            Ok((
                Inspection {
                    format,
                    width: None,
                    height: None,
                    duration: Some(duration),
                },
                Content::Audio(pcm),
            ))
        }
    }
}

fn image_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    limits
}

fn image_error(format: UploadFormat) -> impl Fn(ImageError) -> anyhow::Error {
    move |error| match error {
        ImageError::Limits(_) => invalid(format!(
            "Resize the image to at most {MAX_IMAGE_SIDE} pixels on each side."
        )),
        ImageError::Decoding(_)
        | ImageError::Encoding(_)
        | ImageError::Parameter(_)
        | ImageError::Unsupported(_)
        | ImageError::IoError(_) => damaged(format),
    }
}

fn decode_image(
    format: UploadFormat,
    codec: ImageCodec,
    bytes: &[u8],
) -> anyhow::Result<DynamicImage> {
    let fail = image_error(format);
    // Images are served as still AVIF, and nothing here writes animated AVIF. Refused
    // before any pixels are decoded, rather than cut to their first frame.
    let animated = match codec {
        ImageCodec::Jpeg => false,
        ImageCodec::Png => image::codecs::png::PngDecoder::new(Cursor::new(bytes))
            .map_err(&fail)?
            .is_apng()
            .map_err(&fail)?,
        ImageCodec::WebP => image::codecs::webp::WebPDecoder::new(Cursor::new(bytes))
            .map_err(&fail)?
            .has_animation(),
    };
    if animated {
        return Err(invalid(
            "Animated images are not supported. Choose a still image.",
        ));
    }
    let mut decoder = image::ImageReader::with_format(Cursor::new(bytes), codec.format())
        .into_decoder()
        .map_err(&fail)?;
    decoder.set_limits(image_limits()).map_err(&fail)?;
    // Phones store photos sideways and say so in EXIF; the served file has no EXIF.
    let orientation = decoder.orientation().map_err(&fail)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(&fail)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// Symphonia's decoders plus the reference Opus decoder.
static CODECS: LazyLock<CodecRegistry> = LazyLock::new(|| {
    let mut registry = CodecRegistry::new();
    symphonia::default::register_enabled_codecs(&mut registry);
    registry.register_audio_decoder::<OpusDecoder>();
    registry
});

/// How long `units` of a track's time base last.
fn elapsed(time_base: Option<TimeBase>, units: u64) -> Option<Duration> {
    let time = time_base?.calc_time(Timestamp::new(i64::try_from(units).ok()?))?;
    u64::try_from(time.as_nanos())
        .ok()
        .map(Duration::from_nanos)
}

/// Demux every packet of the clip's audio track and decode each one. Returns the
/// measured length and the decoded samples.
fn decode_audio(
    format: UploadFormat,
    container: Container,
    bytes: &[u8],
) -> anyhow::Result<(Duration, Pcm)> {
    let too_long = || {
        invalid(format!(
            "Audio clips must be {} minutes or shorter.",
            MAX_AUDIO.as_secs() / 60
        ))
    };
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut reader = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| damaged(format))?;
    if reader.format_info().short_name != container.probed_as() {
        return Err(not_the_format(format));
    }
    // `CodecParameters` is `#[non_exhaustive]`, so this cannot list every variant.
    if reader
        .tracks()
        .iter()
        .any(|track| matches!(track.codec_params, Some(CodecParameters::Video(_))))
    {
        return Err(invalid(
            "Video files are not supported. Choose an audio clip.",
        ));
    }
    let track = reader
        .default_track(TrackType::Audio)
        .ok_or_else(|| damaged(format))?;
    let track_id = track.id;
    let time_base = track.time_base;
    let declared_frames = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(CodecParameters::audio)
        .ok_or_else(|| damaged(format))?
        .clone();
    if params
        .sample_rate
        .is_some_and(|rate| !(8_000..=192_000).contains(&rate))
        || params
            .channels
            .as_ref()
            .is_some_and(|channels| channels.count() > 8)
    {
        return Err(damaged(format));
    }
    let mut decoder = CODECS
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|_| {
            invalid(format!(
                "The {} uses an audio codec Keepsake cannot decode.",
                format.name()
            ))
        })?;

    let (mut packets, mut decoded, mut errors, mut units) = (0usize, 0usize, 0usize, 0u64);
    let mut pcm: Option<Pcm> = None;
    let mut interleaved = Vec::new();
    loop {
        let packet = match reader.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(_) => return Err(damaged(format)),
        };
        if packet.track_id != track_id {
            continue;
        }
        packets += 1;
        units = units.saturating_add(packet.dur.get());
        // Stop before decoding anything past the limit.
        if elapsed(time_base, units).is_some_and(|length| length > MAX_AUDIO) {
            return Err(too_long());
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(AudioError::DecodeError(_)) => {
                errors += 1;
                continue;
            }
            // Symphonia's error is `#[non_exhaustive]`; any other failure is fatal.
            Err(_) => return Err(damaged(format)),
        };
        if buffer.frames() == 0 {
            continue;
        }
        decoded += 1;
        let rate = buffer.spec().rate();
        let channels = buffer.spec().channels().count();
        let layout = match channels {
            1 => Layout::Mono,
            2..=8 => Layout::Stereo,
            _ => return Err(damaged(format)),
        };
        let pcm = pcm.get_or_insert_with(|| Pcm {
            rate,
            layout,
            samples: Vec::new(),
        });
        if pcm.rate != rate || pcm.layout != layout {
            return Err(damaged(format));
        }
        interleaved.clear();
        buffer.copy_to_vec_interleaved::<i16>(&mut interleaved);
        // Priming and padding the container marks for removal are not part of the clip.
        let frames = interleaved.len() / channels;
        let start = (packet.trim_start.get() as usize).min(frames);
        let end = frames
            .saturating_sub(packet.trim_end.get() as usize)
            .max(start);
        for frame in interleaved[start * channels..end * channels].chunks_exact(channels) {
            pcm.samples.extend_from_slice(&frame[..layout.channels()]);
        }
    }
    if packets == 0 || decoded == 0 || errors > TOLERATED_AUDIO_ERRORS {
        return Err(damaged(format));
    }
    let pcm = pcm.ok_or_else(|| damaged(format))?;
    let length = elapsed(time_base, units)
        .filter(|length| !length.is_zero())
        .or_else(|| declared_frames.and_then(|frames| elapsed(time_base, frames)))
        .unwrap_or_else(|| Duration::from_secs_f64(pcm.frames() as f64 / f64::from(pcm.rate)));
    if length > MAX_AUDIO {
        return Err(too_long());
    }
    Ok((length, downsample(pcm)))
}

/// Halve or quarter rates above 48 kHz by averaging neighbouring frames. Higher rates hold
/// nothing a listener hears, and AAC is served at 48 kHz at most.
fn downsample(pcm: Pcm) -> Pcm {
    let factor = match pcm.rate {
        88_200 | 96_000 => 2,
        176_400 | 192_000 => 4,
        _ => return pcm,
    };
    let Pcm {
        rate,
        layout,
        samples,
    } = pcm;
    let channels = layout.channels();
    let mut halved = Vec::with_capacity(samples.len() / factor);
    for group in samples.chunks_exact(channels * factor) {
        for channel in 0..channels {
            let sum: i32 = (0..factor)
                .map(|frame| i32::from(group[frame * channels + channel]))
                .sum();
            halved.push((sum / factor as i32) as i16);
        }
    }
    Pcm {
        rate: rate / factor as u32,
        layout,
        samples: halved,
    }
}

#[cfg(test)]
mod tests {
    use strum::IntoEnumIterator;

    use super::*;

    #[test]
    fn signatures_name_only_the_formats_that_are_accepted() {
        for (bytes, expected) in [
            (b"\xFF\xD8\xFF\xE0".as_slice(), Some(UploadFormat::Jpeg)),
            (b"\x89PNG\r\n\x1A\n", Some(UploadFormat::Png)),
            (b"GIF89a", None),
            (b"RIFF\0\0\0\0WEBPVP8 ", Some(UploadFormat::WebP)),
            (b"RIFF\0\0\0\0WAVEfmt ", Some(UploadFormat::Wav)),
            (b"OggS\0", Some(UploadFormat::Ogg)),
            (b"fLaC", Some(UploadFormat::Flac)),
            (b"ID3\x04", Some(UploadFormat::Mp3)),
            (b"\xFF\xFB\x90\x00", Some(UploadFormat::Mp3)),
            (b"\xFF\xF1\x50\x80", Some(UploadFormat::Aac)),
            (b"\0\0\0\x20ftypM4A ", Some(UploadFormat::M4a)),
            (
                b"\x1A\x45\xDF\xA3\x9F\x42\x86\x81\x01\x42\x82\x84webm",
                Some(UploadFormat::WebM),
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
            assert_eq!(UploadFormat::sniff(bytes), expected, "{bytes:?}");
        }
    }

    #[test]
    fn declared_types_accept_real_aliases_and_nothing_else() {
        for (declared, expected) in [
            ("image/JPEG", Some(UploadFormat::Jpeg)),
            ("audio/x-m4a", Some(UploadFormat::M4a)),
            ("audio/webm;codecs=opus", Some(UploadFormat::WebM)),
            ("video/webm", Some(UploadFormat::WebM)),
            ("audio/x-wav", Some(UploadFormat::Wav)),
            ("image/gif", None),
            ("image/svg+xml", None),
            ("text/html", None),
            ("video/mp4", None),
            ("application/octet-stream", None),
            ("", None),
        ] {
            assert_eq!(
                UploadFormat::from_content_type(declared),
                expected,
                "{declared}"
            );
        }
    }

    /// Every format is named in the message an unsupported upload gets, and every image
    /// format decodes as an image and every clip as audio.
    #[test]
    fn every_format_is_offered_and_decoded_as_its_kind() {
        for format in UploadFormat::iter() {
            let short = format.name().split(' ').next().unwrap();
            assert!(SUPPORTED_FORMATS.contains(short), "{format:?}");
            let decoded_as = match format.decoder() {
                Decoder::Image(_) => MediaKind::Image,
                Decoder::Audio(_) => MediaKind::Audio,
            };
            assert_eq!(decoded_as, format.kind(), "{format:?}");
        }
    }
}
