//! Small, genuinely decodable media for upload tests, built here so no third-party
//! recordings are committed. M4A, ADTS AAC and FLAC fixtures were encoded once with
//! macOS `afconvert` from a generated 440 Hz tone and live in `fixtures/media`.
#![allow(dead_code)]

use std::io::Cursor;

use image::{ImageFormat, RgbImage};

pub const M4A: &[u8] = include_bytes!("../fixtures/media/tone.m4a");
pub const AAC: &[u8] = include_bytes!("../fixtures/media/tone.aac");
pub const FLAC: &[u8] = include_bytes!("../fixtures/media/tone.flac");

/// An image whose pixels depend on `seed`, so different seeds hash differently.
pub fn image(format: ImageFormat, width: u32, height: u32, seed: u8) -> Vec<u8> {
    let pixels = RgbImage::from_fn(width, height, |x, y| {
        image::Rgb([seed, (x % 256) as u8, (y % 256) as u8])
    });
    let mut out = Cursor::new(Vec::new());
    pixels.write_to(&mut out, format).unwrap();
    out.into_inner()
}

pub fn png(seed: u8) -> Vec<u8> {
    image(ImageFormat::Png, 4, 3, seed)
}

/// A half-transparent PNG.
pub fn translucent_png() -> Vec<u8> {
    let pixels = image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([200, x as u8, y as u8, 128]));
    let mut out = Cursor::new(Vec::new());
    pixels.write_to(&mut out, ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A JPEG stored sideways, with EXIF orientation 6 saying to turn it 90° clockwise.
pub fn sideways_jpeg(width: u32, height: u32) -> Vec<u8> {
    let jpeg = image(ImageFormat::Jpeg, width, height, 50);
    // "Exif", then a big-endian TIFF header whose first IFD starts at offset 8.
    let mut exif = b"Exif\0\0MM\0\x2A\0\0\0\x08".to_vec();
    exif.extend_from_slice(&[0, 1]); // one entry
    exif.extend_from_slice(&[0x01, 0x12, 0, 3, 0, 0, 0, 1, 0, 6, 0, 0]); // Orientation = 6
    exif.extend_from_slice(&[0, 0, 0, 0]); // no further IFD
    let mut segment = vec![0xFF, 0xE1];
    segment.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    segment.extend(exif);
    [&jpeg[..2], &segment, &jpeg[2..]].concat()
}

/// A GIF's signature and header: refused whatever follows.
pub const GIF: &[u8] = b"GIF89a\x01\x00\x01\x00\x00\x00\x00;";

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let body = [&kind[..], data].concat();
    [
        &(data.len() as u32).to_be_bytes()[..],
        &body,
        &crc32(&body).to_be_bytes(),
    ]
    .concat()
}

/// A two-frame animated PNG: a real PNG with APNG's control and frame chunks added.
pub fn animated_png() -> Vec<u8> {
    let (width, height) = (4u32, 3u32);
    let png = image(ImageFormat::Png, width, height, 9);
    // The encoder writes the signature, IHDR, then IDAT; find IDAT's data.
    let idat = png.windows(4).position(|window| window == b"IDAT").unwrap();
    let length = u32::from_be_bytes(png[idat - 4..idat].try_into().unwrap()) as usize;
    let data = &png[idat + 4..idat + 4 + length];
    let control = |sequence: u32| {
        let mut fctl = sequence.to_be_bytes().to_vec();
        for value in [width, height, 0, 0] {
            fctl.extend_from_slice(&value.to_be_bytes());
        }
        fctl.extend_from_slice(&[0, 1, 0, 10, 0, 0]); // 1/10 s; no disposal or blending
        png_chunk(b"fcTL", &fctl)
    };
    let actl = [&2u32.to_be_bytes()[..], &0u32.to_be_bytes()].concat();
    [
        png[..idat - 4].to_vec(),
        png_chunk(b"acTL", &actl),
        control(0),
        png_chunk(b"IDAT", data),
        control(1),
        png_chunk(b"fdAT", &[&2u32.to_be_bytes()[..], data].concat()),
        png_chunk(b"IEND", &[]),
    ]
    .concat()
}

fn riff_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = [&kind[..], &(data.len() as u32).to_le_bytes(), data].concat();
    if data.len() % 2 == 1 {
        chunk.push(0);
    }
    chunk
}

fn u24(value: u32) -> [u8; 3] {
    let bytes = value.to_le_bytes();
    [bytes[0], bytes[1], bytes[2]]
}

/// A two-frame animated WebP: a real lossless frame, twice, in WebP's animation chunks.
pub fn animated_webp() -> Vec<u8> {
    let (width, height) = (4u32, 3u32);
    let still = image(ImageFormat::WebP, width, height, 9);
    let frame = &still[12..]; // the VP8L chunk, after "RIFF", size and "WEBP"
    let mut vp8x = vec![0x02, 0, 0, 0]; // animation flag
    vp8x.extend(u24(width - 1));
    vp8x.extend(u24(height - 1));
    let anim = [0u8, 0, 0, 0, 0, 0]; // background colour, loop forever
    let anmf = || {
        let mut header = Vec::new();
        for value in [0, 0, width - 1, height - 1, 100] {
            header.extend(u24(value));
        }
        header.push(0);
        riff_chunk(b"ANMF", &[&header[..], frame].concat())
    };
    let body = [
        b"WEBP".to_vec(),
        riff_chunk(b"VP8X", &vp8x),
        riff_chunk(b"ANIM", &anim),
        anmf(),
        anmf(),
    ]
    .concat();
    [&b"RIFF"[..], &(body.len() as u32).to_le_bytes(), &body].concat()
}

/// 16-bit mono PCM at 16 kHz.
pub fn wav(milliseconds: u32, seed: u16) -> Vec<u8> {
    pcm_wav(16_000, 1, 16, milliseconds, |index| {
        let sample = ((index as u16).wrapping_mul(seed | 1) % 2000) as i16 - 1000;
        sample.to_le_bytes().to_vec()
    })
}

/// 16-bit PCM at any rate and channel count: a 440 Hz tone in every channel.
pub fn tone(rate: u32, channels: u16, milliseconds: u32) -> Vec<u8> {
    pcm_wav(rate, channels, 16, milliseconds, |index| {
        let phase = f64::from(index) * 440.0 * std::f64::consts::TAU / f64::from(rate);
        let sample = ((phase.sin() * 8000.0) as i16).to_le_bytes();
        sample.repeat(usize::from(channels))
    })
}

/// 8-bit mono PCM at 8 kHz: the cheapest way to make a long clip under the size limit.
pub fn long_wav(milliseconds: u32) -> Vec<u8> {
    pcm_wav(8_000, 1, 8, milliseconds, |index| {
        vec![(index % 200) as u8 + 28]
    })
}

fn pcm_wav(
    rate: u32,
    channels: u16,
    bits: u16,
    milliseconds: u32,
    frame: impl Fn(u32) -> Vec<u8>,
) -> Vec<u8> {
    let frames = (u64::from(rate) * u64::from(milliseconds) / 1000) as u32;
    let data: Vec<u8> = (0..frames).flat_map(frame).collect();
    let bytes_per_frame = u32::from(bits / 8) * u32::from(channels);
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * bytes_per_frame).to_le_bytes());
    out.extend_from_slice(&(bytes_per_frame as u16).to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

/// Silent MPEG-1 Layer III frames: 128 kbit/s, 44.1 kHz, mono, zeroed side information.
/// Each frame is 417 bytes and 1152 samples (about 26 ms).
pub fn mp3(frames: usize) -> Vec<u8> {
    let mut frame = vec![0u8; 417];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0xC0]);
    frame.repeat(frames)
}

fn opus_head() -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.push(1); // version
    head.push(1); // channels
    head.extend_from_slice(&312u16.to_le_bytes()); // pre-skip
    head.extend_from_slice(&48_000u32.to_le_bytes());
    head.extend_from_slice(&0i16.to_le_bytes()); // output gain
    head.push(0); // mapping family
    head
}

/// A 20 ms CELT fullband mono Opus packet. Its payload is never decoded here.
const OPUS_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];

fn ogg_crc(data: &[u8]) -> u32 {
    let mut crc = 0u32;
    for byte in data {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04C1_1DB7
            } else {
                crc << 1
            };
        }
    }
    crc
}

fn ogg_page(header_type: u8, granule: i64, sequence: u32, packets: &[&[u8]]) -> Vec<u8> {
    let mut lacing = Vec::new();
    for packet in packets {
        let mut remaining = packet.len();
        while remaining >= 255 {
            lacing.push(255);
            remaining -= 255;
        }
        lacing.push(remaining as u8);
    }
    let mut page = b"OggS".to_vec();
    page.push(0);
    page.push(header_type);
    page.extend_from_slice(&granule.to_le_bytes());
    page.extend_from_slice(&0x4B53_5045u32.to_le_bytes()); // serial
    page.extend_from_slice(&sequence.to_le_bytes());
    page.extend_from_slice(&[0; 4]); // checksum, filled below
    page.push(lacing.len() as u8);
    page.extend_from_slice(&lacing);
    for packet in packets {
        page.extend_from_slice(packet);
    }
    let crc = ogg_crc(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

/// Ogg Opus with `packets` 20 ms packets.
pub fn ogg_opus(packets: usize) -> Vec<u8> {
    let mut tags = b"OpusTags".to_vec();
    tags.extend_from_slice(&8u32.to_le_bytes());
    tags.extend_from_slice(b"keepsake");
    tags.extend_from_slice(&0u32.to_le_bytes());
    let audio = vec![OPUS_PACKET.as_slice(); packets];
    [
        ogg_page(0x02, 0, 0, &[&opus_head()]),
        ogg_page(0x00, 0, 1, &[&tags]),
        ogg_page(0x04, 312 + 960 * packets as i64, 2, &audio),
    ]
    .concat()
}

/// The shortest size encoding, as browser recorders and muxers write it.
fn ebml_size(length: usize) -> Vec<u8> {
    match length {
        0..=126 => vec![0x80 | length as u8],
        127..=16_382 => vec![0x40 | (length >> 8) as u8, length as u8],
        _ => {
            let mut size = (length as u64).to_be_bytes();
            size[0] = 0x01;
            size.to_vec()
        }
    }
}

fn element(id: &[u8], body: &[u8]) -> Vec<u8> {
    [id, &ebml_size(body.len()), body].concat()
}

fn uint(id: &[u8], value: u64) -> Vec<u8> {
    element(id, &value.to_be_bytes())
}

/// WebM holding one Opus track with `packets` 20 ms blocks, as a browser recorder writes.
pub fn webm_opus(packets: usize) -> Vec<u8> {
    webm(packets, false)
}

/// The same, plus a VP8 video track.
pub fn webm_with_video(packets: usize) -> Vec<u8> {
    webm(packets, true)
}

fn webm(packets: usize, with_video: bool) -> Vec<u8> {
    let header = element(
        &[0x1A, 0x45, 0xDF, 0xA3],
        &[
            uint(&[0x42, 0x86], 1),
            uint(&[0x42, 0xF7], 1),
            uint(&[0x42, 0xF2], 4),
            uint(&[0x42, 0xF3], 8),
            element(&[0x42, 0x82], b"webm"),
            uint(&[0x42, 0x87], 4),
            uint(&[0x42, 0x85], 2),
        ]
        .concat(),
    );
    let info = element(
        &[0x15, 0x49, 0xA9, 0x66],
        &[
            uint(&[0x2A, 0xD7, 0xB1], 1_000_000),
            element(&[0x4D, 0x80], b"keepsake-tests"),
            element(&[0x57, 0x41], b"keepsake-tests"),
        ]
        .concat(),
    );
    let audio = element(
        &[0xE1],
        &[element(&[0xB5], &48_000f64.to_be_bytes()), uint(&[0x9F], 1)].concat(),
    );
    let track = element(
        &[0xAE],
        &[
            uint(&[0xD7], 1),
            uint(&[0x73, 0xC5], 1),
            uint(&[0x83], 2), // audio
            element(&[0x86], b"A_OPUS"),
            uint(&[0x23, 0xE3, 0x83], 20_000_000),
            element(&[0x63, 0xA2], &opus_head()),
            audio,
        ]
        .concat(),
    );
    let video = element(
        &[0xAE],
        &[
            uint(&[0xD7], 2),
            uint(&[0x73, 0xC5], 2),
            uint(&[0x83], 1), // video
            element(&[0x86], b"V_VP8"),
            element(&[0xE0], &[uint(&[0xB0], 64), uint(&[0xBA], 48)].concat()),
        ]
        .concat(),
    );
    let tracks = element(
        &[0x16, 0x54, 0xAE, 0x6B],
        &[track, if with_video { video } else { Vec::new() }].concat(),
    );
    let mut cluster = uint(&[0xE7], 0);
    for index in 0..packets {
        let mut block = vec![0x81];
        block.extend_from_slice(&((index * 20) as i16).to_be_bytes());
        block.push(0x80); // keyframe
        block.extend_from_slice(&OPUS_PACKET);
        cluster.extend(element(&[0xA3], &block));
    }
    let cluster = element(&[0x1F, 0x43, 0xB6, 0x75], &cluster);
    let segment = element(&[0x18, 0x53, 0x80, 0x67], &[info, tracks, cluster].concat());
    [header, segment].concat()
}
