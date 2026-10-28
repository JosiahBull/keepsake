//! The one format each kind of media is served in, made when it is stored.
//!
//! - Images become AVIF, at most 2048 pixels on the long edge: small, and decoded by
//!   every current browser. Animated images are refused before they get here.
//! - Clips become AAC-LC in an MP4, index first. Phones and computers decode AAC in
//!   hardware, it is compact at speech and music bitrates, and the index lets playback
//!   and scrubbing start from the first bytes over range requests.
//!
//! The upload itself is not kept.

use std::time::Duration;

use fdk_aac_rs::encoder::{Aot, Bitrate, ChannelMode, Encoder, EncoderError, Transport};
use image::{DynamicImage, ImageEncoder, imageops::FilterType};

use super::inspect::{Content, Layout, Pcm};
use super::{MAX_MEDIA_BYTES, MediaKind, invalid, mp4};

/// The longest edge served. Sharp on a large phone or laptop screen at 2× density.
pub const MAX_SERVED_SIDE: u32 = 2048;
/// rav1e's speed, 1 (slowest) to 10. 8 was within 3% of 4's size, at 70% of its time.
const AVIF_SPEED: u8 = 8;
/// Visually clean for photos and diagrams at about a tenth of a JPEG's size.
const AVIF_QUALITY: u8 = 70;
/// AAC-LC bitrate for each channel: transparent for speech, good for music.
const AAC_BITS_PER_CHANNEL: u32 = 56_000;

/// A file ready to serve in its kind's one format, and what it holds.
pub(crate) struct Rendered {
    pub kind: MediaKind,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration: Option<Duration>,
    pub data: Vec<u8>,
}

pub(crate) fn render(content: Content) -> anyhow::Result<Rendered> {
    let rendered = match content {
        Content::Still(image) => still(image)?,
        Content::Audio(pcm) => clip(&pcm)?,
    };
    if rendered.data.is_empty() || rendered.data.len() > MAX_MEDIA_BYTES {
        anyhow::bail!(
            "rendered {} is {} bytes",
            rendered.kind.served_type(),
            rendered.data.len()
        );
    }
    Ok(rendered)
}

fn still(image: DynamicImage) -> anyhow::Result<Rendered> {
    let image = if image.width().max(image.height()) > MAX_SERVED_SIDE {
        image.resize(MAX_SERVED_SIDE, MAX_SERVED_SIDE, FilterType::Lanczos3)
    } else {
        image
    };
    let (width, height) = (image.width(), image.height());
    let mut data = Vec::new();
    let encoder = image::codecs::avif::AvifEncoder::new_with_speed_quality(
        &mut data,
        AVIF_SPEED,
        AVIF_QUALITY,
    );
    if image.color().has_alpha() {
        let pixels = image.into_rgba8();
        encoder.write_image(&pixels, width, height, image::ExtendedColorType::Rgba8)?;
    } else {
        let pixels = image.into_rgb8();
        encoder.write_image(&pixels, width, height, image::ExtendedColorType::Rgb8)?;
    }
    Ok(Rendered {
        kind: MediaKind::Image,
        width: Some(width),
        height: Some(height),
        duration: None,
        data,
    })
}

fn clip(pcm: &Pcm) -> anyhow::Result<Rendered> {
    let channel_mode = match pcm.layout {
        Layout::Mono => ChannelMode::Mono,
        Layout::Stereo => ChannelMode::Stereo,
    };
    // Low sample rates cannot carry the full bitrate; AAC allows about six bits a sample.
    let bitrate = AAC_BITS_PER_CHANNEL.min(3 * pcm.rate) * pcm.layout.channels() as u32;
    let mut encoder = Encoder::builder()
        .aot(Aot::AacLc)
        .sample_rate(pcm.rate)
        .channel_mode(channel_mode)
        .bitrate(Bitrate::Cbr(bitrate))
        .transport(Transport::Raw)
        .afterburner(true)
        .build()
        .map_err(|_| {
            invalid(format!(
                "Audio sampled at {} Hz cannot be stored. Use a standard rate such as 44.1 or 48 kHz.",
                pcm.rate
            ))
        })?;
    let info = encoder
        .info()
        .map_err(|error| anyhow::anyhow!("AAC encoder info: {error:?}"))?;
    let mut buffer = vec![0u8; info.max_out_buf_bytes() as usize];
    let mut frames = Vec::new();
    let mut input = &pcm.samples[..];
    while !input.is_empty() {
        let output = encoder
            .encode(input, &mut buffer)
            .map_err(|error| anyhow::anyhow!("AAC encode: {error:?}"))?;
        if output.out_bytes() > 0 {
            frames.push(buffer[..output.out_bytes()].to_vec());
        }
        anyhow::ensure!(
            output.in_samples() > 0 || output.out_bytes() > 0,
            "AAC encoder made no progress"
        );
        input = &input[output.in_samples()..];
    }
    loop {
        match encoder.flush(&mut buffer) {
            Ok(output) if output.out_bytes() > 0 => {
                frames.push(buffer[..output.out_bytes()].to_vec())
            }
            Ok(_) => {}
            Err(EncoderError::EncodeEof) => break,
            Err(error) => anyhow::bail!("AAC flush: {error:?}"),
        }
    }
    let samples = pcm.frames();
    let data = mp4::m4a(&mp4::AacTrack {
        config: info.config(),
        sample_rate: pcm.rate,
        channels: pcm.layout.channels() as u16,
        frame_length: info.frame_length(),
        delay: info.delay(),
        samples,
        frames: &frames,
    });
    Ok(Rendered {
        kind: MediaKind::Audio,
        width: None,
        height: None,
        // Whole milliseconds, rounded up as the MP4's edit list is, so the two agree.
        duration: Some(Duration::from_millis(
            (samples * 1000).div_ceil(u64::from(pcm.rate)),
        )),
        data,
    })
}
