//! A single AAC track in an MP4 (`.m4a`), with its index before the audio.
//!
//! The `moov` box comes first so a browser can start playing, and seek, from the first
//! bytes it receives: every sample's size and position is known before any audio
//! arrives, and range requests fetch the rest. An edit list hides the encoder's priming
//! samples at the start and the padding that flushed its last frame.

/// An encoded AAC track and what describes it.
pub(crate) struct AacTrack<'a> {
    /// The AudioSpecificConfig the encoder reports.
    pub config: &'a [u8],
    pub sample_rate: u32,
    pub channels: u16,
    /// Samples per access unit, 1024 for AAC-LC.
    pub frame_length: u32,
    /// Priming samples at the start of the decoded stream.
    pub delay: u32,
    /// Samples per channel of the source, without priming or padding.
    pub samples: u64,
    pub frames: &'a [Vec<u8>],
}

fn boxed(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(body);
    out
}

fn full(kind: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut header = flags.to_be_bytes();
    header[0] = version;
    boxed(kind, &[&header[..], body].concat())
}

/// An MPEG-4 descriptor with a one-byte length; every descriptor here is short.
fn descriptor(tag: u8, body: &[u8]) -> Vec<u8> {
    debug_assert!(body.len() < 128);
    [&[tag, body.len() as u8][..], body].concat()
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

fn matrix() -> Vec<u8> {
    MATRIX
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect()
}

fn esds(track: &AacTrack<'_>, bytes: u64, seconds: f64) -> Vec<u8> {
    let largest = track.frames.iter().map(Vec::len).max().unwrap_or(0) as u32;
    let peak = (f64::from(largest) * 8.0 * f64::from(track.sample_rate)
        / f64::from(track.frame_length)) as u32;
    let average = if seconds > 0.0 {
        (bytes as f64 * 8.0 / seconds) as u32
    } else {
        0
    };
    let mut config = vec![0x40, 0x15]; // MPEG-4 audio; audio stream
    config.extend_from_slice(&largest.to_be_bytes()[1..]); // bufferSizeDB, 24 bits
    config.extend_from_slice(&peak.to_be_bytes());
    config.extend_from_slice(&average.to_be_bytes());
    config.extend(descriptor(0x05, track.config));
    let mut es = vec![0, 1, 0]; // ES_ID 1, no flags
    es.extend(descriptor(0x04, &config));
    es.extend(descriptor(0x06, &[0x02])); // SLConfig: predefined for MP4
    full(b"esds", 0, 0, &descriptor(0x03, &es))
}

fn moov(track: &AacTrack<'_>, chunk_offset: u32) -> Vec<u8> {
    let count = track.frames.len() as u32;
    let media_duration = u64::from(count) * u64::from(track.frame_length);
    let played_ms = (track.samples * 1000).div_ceil(u64::from(track.sample_rate)) as u32;
    let bytes: u64 = track.frames.iter().map(|frame| frame.len() as u64).sum();

    let mut mvhd = vec![0; 8]; // creation and modification times
    mvhd.extend_from_slice(&1000u32.to_be_bytes()); // timescale: milliseconds
    mvhd.extend_from_slice(&played_ms.to_be_bytes());
    mvhd.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0
    mvhd.extend_from_slice(&0x0100u16.to_be_bytes()); // volume 1.0
    mvhd.extend_from_slice(&[0; 10]);
    mvhd.extend(matrix());
    mvhd.extend_from_slice(&[0; 24]);
    mvhd.extend_from_slice(&2u32.to_be_bytes()); // next track ID

    let mut tkhd = vec![0; 8];
    tkhd.extend_from_slice(&1u32.to_be_bytes()); // track ID
    tkhd.extend_from_slice(&[0; 4]);
    tkhd.extend_from_slice(&played_ms.to_be_bytes());
    tkhd.extend_from_slice(&[0; 8]);
    tkhd.extend_from_slice(&[0; 4]); // layer, alternate group
    tkhd.extend_from_slice(&0x0100u16.to_be_bytes()); // volume
    tkhd.extend_from_slice(&[0; 2]);
    tkhd.extend(matrix());
    tkhd.extend_from_slice(&[0; 8]); // no width or height

    let mut elst = 1u32.to_be_bytes().to_vec();
    elst.extend_from_slice(&played_ms.to_be_bytes());
    elst.extend_from_slice(&track.delay.to_be_bytes()); // media time: skip priming
    elst.extend_from_slice(&0x0001_0000u32.to_be_bytes()); // rate 1.0

    let mut mdhd = vec![0; 8];
    mdhd.extend_from_slice(&track.sample_rate.to_be_bytes());
    mdhd.extend_from_slice(&(media_duration as u32).to_be_bytes());
    mdhd.extend_from_slice(&0x55C4u16.to_be_bytes()); // language "und"
    mdhd.extend_from_slice(&[0; 2]);

    let mut hdlr = vec![0; 4];
    hdlr.extend_from_slice(b"soun");
    hdlr.extend_from_slice(&[0; 12]);
    hdlr.extend_from_slice(b"SoundHandler\0");

    let mut mp4a = vec![0; 6];
    mp4a.extend_from_slice(&1u16.to_be_bytes()); // data reference index
    mp4a.extend_from_slice(&[0; 8]);
    mp4a.extend_from_slice(&track.channels.to_be_bytes());
    mp4a.extend_from_slice(&16u16.to_be_bytes()); // sample size
    mp4a.extend_from_slice(&[0; 4]);
    mp4a.extend_from_slice(&(track.sample_rate << 16).to_be_bytes());
    mp4a.extend(esds(
        track,
        bytes,
        track.samples as f64 / f64::from(track.sample_rate),
    ));
    let mut stsd = 1u32.to_be_bytes().to_vec();
    stsd.extend(boxed(b"mp4a", &mp4a));

    let mut stts = 1u32.to_be_bytes().to_vec();
    stts.extend_from_slice(&count.to_be_bytes());
    stts.extend_from_slice(&track.frame_length.to_be_bytes());

    // One chunk holding every sample; the size table locates each within it.
    let mut stsc = 1u32.to_be_bytes().to_vec();
    stsc.extend_from_slice(&1u32.to_be_bytes());
    stsc.extend_from_slice(&count.to_be_bytes());
    stsc.extend_from_slice(&1u32.to_be_bytes());

    let mut stsz = 0u32.to_be_bytes().to_vec();
    stsz.extend_from_slice(&count.to_be_bytes());
    for frame in track.frames {
        stsz.extend_from_slice(&(frame.len() as u32).to_be_bytes());
    }

    let mut stco = 1u32.to_be_bytes().to_vec();
    stco.extend_from_slice(&chunk_offset.to_be_bytes());

    let stbl = boxed(
        b"stbl",
        &[
            full(b"stsd", 0, 0, &stsd),
            full(b"stts", 0, 0, &stts),
            full(b"stsc", 0, 0, &stsc),
            full(b"stsz", 0, 0, &stsz),
            full(b"stco", 0, 0, &stco),
        ]
        .concat(),
    );
    let dinf = boxed(
        b"dinf",
        &full(
            b"dref",
            0,
            0,
            &[&1u32.to_be_bytes()[..], &full(b"url ", 0, 1, &[])].concat(),
        ),
    );
    let minf = boxed(
        b"minf",
        &[full(b"smhd", 0, 0, &[0; 4]), dinf, stbl].concat(),
    );
    let mdia = boxed(
        b"mdia",
        &[full(b"mdhd", 0, 0, &mdhd), full(b"hdlr", 0, 0, &hdlr), minf].concat(),
    );
    let trak = boxed(
        b"trak",
        &[
            full(b"tkhd", 0, 0x3, &tkhd), // enabled, in movie
            boxed(b"edts", &full(b"elst", 0, 0, &elst)),
            mdia,
        ]
        .concat(),
    );
    boxed(b"moov", &[full(b"mvhd", 0, 0, &mvhd), trak].concat())
}

/// The `.m4a` file.
pub(crate) fn m4a(track: &AacTrack<'_>) -> Vec<u8> {
    let mut ftyp = b"M4A ".to_vec();
    ftyp.extend_from_slice(&0x200u32.to_be_bytes());
    for brand in [b"M4A ", b"mp42", b"isom"] {
        ftyp.extend_from_slice(brand);
    }
    let ftyp = boxed(b"ftyp", &ftyp);
    // The offset does not change the index's size, so measure it, then write it for real.
    let index_size = moov(track, 0).len();
    let audio_start = (ftyp.len() + index_size + 8) as u32;
    let audio: Vec<u8> = track.frames.concat();
    [ftyp, moov(track, audio_start), boxed(b"mdat", &audio)].concat()
}
