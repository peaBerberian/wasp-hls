// NOTE/TODO: this is a work in progress to re-implement the TypeScript transmuxing logic into Rust,
// it is being done, from the beginning of the pipeline to its end.
//
// None of those files are ready, nor optimized, nor used for the moment. You're very welcome to
// improve it.

use super::{
    frame_utils::{FrameObject, GopsSet},
    nal_unit_producer::NalVideoProperties,
};

static AVC1: [u8; 4] = [97, 118, 99, 49];
static AVCC: [u8; 4] = [97, 118, 99, 67];
static BTRT: [u8; 4] = [98, 116, 114, 116];
static DINF: [u8; 4] = [100, 105, 110, 102];
static DREF: [u8; 4] = [100, 114, 101, 102];
static ESDS: [u8; 4] = [101, 115, 100, 115];
static FTYP: [u8; 4] = [102, 116, 121, 112];
static HDLR: [u8; 4] = [104, 100, 108, 114];
static MDAT: [u8; 4] = [109, 100, 97, 116];
static MDHD: [u8; 4] = [109, 100, 104, 100];
static MDIA: [u8; 4] = [109, 100, 105, 97];
static MFHD: [u8; 4] = [109, 102, 104, 100];
static MINF: [u8; 4] = [109, 105, 110, 102];
static MOOF: [u8; 4] = [109, 111, 111, 102];
static MOOV: [u8; 4] = [109, 111, 111, 118];
static MP4A: [u8; 4] = [109, 112, 52, 97];
static MVEX: [u8; 4] = [109, 118, 101, 120];
static MVHD: [u8; 4] = [109, 118, 104, 100];
static PASP: [u8; 4] = [112, 97, 115, 112];
static SDTP: [u8; 4] = [115, 100, 116, 112];
static SMHD: [u8; 4] = [115, 109, 104, 100];
static STBL: [u8; 4] = [115, 116, 98, 108];
static STCO: [u8; 4] = [115, 116, 99, 111];
static STSC: [u8; 4] = [115, 116, 115, 99];
static STSD: [u8; 4] = [115, 116, 115, 100];
static STSZ: [u8; 4] = [115, 116, 115, 122];
static STTS: [u8; 4] = [115, 116, 116, 115];
static STYP: [u8; 4] = [115, 116, 121, 112];
static TFDT: [u8; 4] = [116, 102, 100, 116];
static TFHD: [u8; 4] = [116, 102, 104, 100];
static TRAF: [u8; 4] = [116, 114, 97, 102];
static TRAK: [u8; 4] = [116, 114, 97, 107];
static TRUN: [u8; 4] = [116, 114, 117, 110];
static TREX: [u8; 4] = [116, 114, 101, 120];
static TKHD: [u8; 4] = [116, 107, 104, 100];
static VMHD: [u8; 4] = [118, 109, 104, 100];

/// Create a box with the given name and corresponding content.
fn create_box(box_name: [u8; 4], children: &[Vec<u8>]) -> Vec<u8> {
    let len = children.iter().fold(0, |acc, c| acc + c.len()) + 4 + 4;
    let mut result = Vec::with_capacity(len);
    result.extend(box_name);
    result.extend((len as u32).to_be_bytes());
    children.iter().for_each(|v| {
        result.extend(v);
    });
    result
}

/// Create a template `dinf` ISOBMFF box.
fn create_dinf() -> Vec<u8> {
    let dref_content: Vec<u8> = vec![
        0x00, // version 0
        0x00, 0x00, 0x00, // flags
        0x00, 0x00, 0x00, 0x01, // entry_count
        0x00, 0x00, 0x00, 0x0c, // entry_size
        0x75, 0x72, 0x6c, 0x20, // 'url' type
        0x00, // version 0
        0x00, 0x00, 0x01, // entry_flags
    ];
    create_box(DINF, &[create_box(DREF, &[dref_content])])
}

/// Create an `esds` ISOBMFF box.
fn create_esds(audio_object_type: u8, sampling_frequency_index: u8, channel_count: u16) -> Vec<u8> {
    create_box(
        ESDS,
        &[vec![
            0x00, // version
            0x00,
            0x00,
            0x00, // flags
            // ES_Descriptor
            0x03, // tag, ES_DescrTag
            0x19, // length
            0x00,
            0x00, // ES_ID
            0x00, // streamDependenceFlag, URL_flag, reserved, streamPriority
            // DecoderConfigDescriptor
            0x04, // tag, DecoderConfigDescrTag
            0x11, // length
            0x40, // object type
            0x15, // streamType
            0x00,
            0x06,
            0x00, // bufferSizeDB
            0x00,
            0x00,
            0xda,
            0xc0, // maxBitrate
            0x00,
            0x00,
            0xda,
            0xc0, // avgBitrate
            // DecoderSpecificInfo
            0x05, // tag, DecoderSpecificInfoTag
            0x02, // length
            // ISO/IEC 14496-3, AudioSpecificConfig
            // for samplingFrequencyIndex see ISO/IEC 13818-7:2006, 8.1.3.2.2, Table 35
            (audio_object_type << 3) | (sampling_frequency_index >> 1),
            (sampling_frequency_index << 7) | ((channel_count << 3) as u8),
            0x06,
            0x01,
            0x02, // GASpecificConfig
        ]],
    )
}

/// Create a template `ftyp` ISOBMFF box.
fn create_ftyp() -> Vec<u8> {
    let major_brand: Vec<u8> = vec![105, 115, 111, 109];
    let avc1_brand: Vec<u8> = vec![97, 118, 99, 49];
    let minor_version: Vec<u8> = vec![0, 0, 0, 1];
    create_box(
        FTYP,
        &[major_brand.clone(), minor_version, major_brand, avc1_brand],
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MediaType {
    Audio,
    Video,
}

/// Create an `hdlr` ISOBMFF box.
fn create_hdlr(hdlr_type: MediaType) -> Vec<u8> {
    let video_hdlr: Vec<u8> = vec![
        0x00, // version 0
        0x00, 0x00, 0x00, // flags
        0x00, 0x00, 0x00, 0x00, // pre_defined
        0x76, 0x69, 0x64, 0x65, // handler_type: 'vide'
        0x00, 0x00, 0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x00, // reserved
        0x56, 0x69, 0x64, 0x65, 0x6f, 0x48, 0x61, 0x6e, 0x64, 0x6c, 0x65, 0x72,
        0x00, // name: 'VideoHandler'
    ];
    let audio_hdlr: Vec<u8> = vec![
        0x00, // version 0
        0x00, 0x00, 0x00, // flags
        0x00, 0x00, 0x00, 0x00, // pre_defined
        0x73, 0x6f, 0x75, 0x6e, // handler_type: 'soun'
        0x00, 0x00, 0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x00, // reserved
        0x00, 0x00, 0x00, 0x00, // reserved
        0x53, 0x6f, 0x75, 0x6e, 0x64, 0x48, 0x61, 0x6e, 0x64, 0x6c, 0x65, 0x72,
        0x00, // name: 'SoundHandler'
    ];
    match hdlr_type {
        MediaType::Audio => create_box(HDLR, &[audio_hdlr]),
        MediaType::Video => create_box(HDLR, &[video_hdlr]),
    }
}

/// Create a `mdat` ISOBMFF box with the corresponding data.
pub(super) fn create_mdat(data: Vec<u8>) -> Vec<u8> {
    create_box(MDAT, &[data])
}

/// Create a `mdhd` ISOBMFF box.
fn create_mdhd(duration: u32, sample_rate: Option<u32>) -> Vec<u8> {
    let mut result: Vec<u8> = vec![
        0x00, // version 0
        0x00,
        0x00,
        0x00, // flags
        0x00,
        0x00,
        0x00,
        0x02, // creation_time
        0x00,
        0x00,
        0x00,
        0x03, // modification_time
        0x00,
        0x01,
        0x5f,
        0x90, // timescale, 90,000 "ticks" per second
        ((duration >> 24) & 0xff) as u8,
        ((duration >> 16) & 0xff) as u8,
        ((duration >> 8) & 0xff) as u8,
        (duration & 0xff) as u8, // duration
        0x55,
        0xc4, // 'und' language (undetermined)
        0x00,
        0x00,
    ];

    // Use the sample rate from the metadata, when it is
    // defined. The sample rate can be parsed out of an ADTS header, for
    // instance.
    if let Some(sample_rate) = sample_rate {
        result[12] = ((sample_rate >> 24) & 0xff) as u8;
        result[13] = ((sample_rate >> 16) & 0xff) as u8;
        result[14] = ((sample_rate >> 8) & 0xff) as u8;
        result[15] = (sample_rate & 0xff) as u8;
    }
    create_box(MDHD, &[result])
}

/// Create a `mdia` ISOBMFF box.
fn create_mdia(md: &IsobmffMetadata, duration: u32, sample_rate: Option<u32>) -> Vec<u8> {
    create_box(
        MDIA,
        &[
            create_mdhd(duration, sample_rate),
            create_hdlr(md.media_type()),
            create_minf(md),
        ],
    )
}

/// Create a `mfhd` ISOBMFF box.
fn create_mfhd(sequence_number: u32) -> Vec<u8> {
    create_box(
        MFHD,
        &[vec![
            0x00,
            0x00,
            0x00,
            0x00, // flags
            ((sequence_number & 0xff000000) >> 24) as u8,
            ((sequence_number & 0xff0000) >> 16) as u8,
            ((sequence_number & 0xff00) >> 8) as u8,
            (sequence_number & 0xff) as u8, // sequence_number
        ]],
    )
}

/// Create a `minf` ISOBMFF box.
fn create_minf(md: &IsobmffMetadata) -> Vec<u8> {
    let smhd_content: Vec<u8> = vec![
        0x00, // version
        0x00, 0x00, 0x00, // flags
        0x00, 0x00, // balance, 0 means centered
        0x00, 0x00, // reserved
    ];
    let vmhd_content: Vec<u8> = vec![
        0x00, // version
        0x00, 0x00, 0x01, // flags
        0x00, 0x00, // graphicsmode
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // opcolor
    ];
    create_box(
        MINF,
        &[
            match md {
                IsobmffMetadata::Video(_) => create_box(VMHD, &[vmhd_content]),
                IsobmffMetadata::Audio(_) => create_box(SMHD, &[smhd_content]),
            },
            create_dinf(),
            create_stbl(md),
        ],
    )
}

/// Create a `moof` ISOBMFF box.
pub(super) fn create_moof(sequence_number: u32, tracks: &[TrackInfo]) -> Vec<u8> {
    let track_fragments: Vec<Vec<u8>> = tracks.iter().map(create_traf).collect();
    let mut content = vec![create_mfhd(sequence_number)];
    content.extend(track_fragments);
    create_box(MOOF, &content)
}

pub(super) struct TrackInfo {
    md: IsobmffMetadata,
    track_id: u32,
    base_media_decode_time: u32,
    duration: Option<u32>,
    samples: Vec<SampleInfo>,
    sample_rate: Option<u32>,
}

/// Creates a `moov` ISOBMFF box.
fn create_moov(tracks: &[TrackInfo]) -> Vec<u8> {
    let mut inner_boxes = vec![create_mvhd(0xffffffff)];
    tracks
        .iter()
        .for_each(|t| inner_boxes.push(create_trak(&t.md, t.track_id, t.duration, t.sample_rate)));
    inner_boxes.push(create_mvex(tracks));
    create_box(MOOV, &inner_boxes)
}

/// Creates a `mvex` ISOBMFF box.
fn create_mvex(tracks: &[TrackInfo]) -> Vec<u8> {
    let boxes: Vec<Vec<u8>> = tracks
        .iter()
        .map(|t| create_trex(t.md.media_type(), t.track_id))
        .collect();
    create_box(MVEX, &boxes)
}

/// Creates a `mvhd` ISOBMFF box.
fn create_mvhd(duration: u32) -> Vec<u8> {
    let bytes: Vec<u8> = vec![
        0x00, // version 0
        0x00,
        0x00,
        0x00, // flags
        0x00,
        0x00,
        0x00,
        0x01, // creation_time
        0x00,
        0x00,
        0x00,
        0x02, // modification_time
        0x00,
        0x01,
        0x5f,
        0x90, // timescale, 90,000 "ticks" per second
        ((duration & 0xff000000) >> 24) as u8,
        ((duration & 0xff0000) >> 16) as u8,
        ((duration & 0xff00) >> 8) as u8,
        (duration & 0xff) as u8, // duration
        0x00,
        0x01,
        0x00,
        0x00, // 1.0 rate
        0x01,
        0x00, // 1.0 volume
        0x00,
        0x00, // reserved
        0x00,
        0x00,
        0x00,
        0x00, // reserved
        0x00,
        0x00,
        0x00,
        0x00, // reserved
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x40,
        0x00,
        0x00,
        0x00, // transformation: unity matrix
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00, // pre_defined
        0xff,
        0xff,
        0xff,
        0xff, // next_track_ID
    ];
    create_box(MVHD, &[bytes])
}

/// Creates a `sdtp` ISOBMFF box.
fn create_sdtp(samples: &[SampleInfo]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + samples.len());

    // leave the full box header (4 bytes) all zero

    // write the sample table
    for i in 0..samples.len() {
        let flags = &samples[i].flags;
        bytes[i + 4] = (flags.depends_on << 4) | (flags.is_depended_on << 2) | flags.has_redundancy;
    }

    create_box(SDTP, &[bytes])
}

/// Creates a `stbl` ISOBMFF box.
fn create_stbl(md: &IsobmffMetadata) -> Vec<u8> {
    let empty_content: Vec<u8> = vec![
        0x00, // version
        0x00, 0x00, 0x00, // flags
        0x00, 0x00, 0x00, 0x00, // entry_count
    ];
    create_box(
        STBL,
        &[
            create_stsd(md),
            create_box(STTS, &[empty_content.clone()]),
            create_box(STSC, &[empty_content.clone()]),
            create_box(STSZ, &[empty_content.clone()]),
            create_box(STCO, &[empty_content]),
        ],
    )
}

pub(super) enum IsobmffMetadata {
    Video(VideoMetadata),
    Audio(AudioMetadata),
}

impl IsobmffMetadata {
    fn media_type(&self) -> MediaType {
        match self {
            IsobmffMetadata::Video(_) => MediaType::Video,
            IsobmffMetadata::Audio(_) => MediaType::Audio,
        }
    }
}

pub(super) struct VideoMetadata {
    nal_video_properties: NalVideoProperties,
    ppss: Vec<Vec<u8>>,
    spss: Vec<Vec<u8>>,
}

pub(super) struct AudioMetadata {
    audio_object_type: u8,
    sampling_frequency_index: u8,
    channel_count: u16,
    sample_size: u32,
    sample_rate: u32,
}

/// Creates a `stsd` ISOBMFF box.
fn create_stsd(md: &IsobmffMetadata) -> Vec<u8> {
    create_box(
        STSD,
        &[
            vec![
                0x00, // version 0
                0x00, 0x00, 0x00, // flags
                0x00, 0x00, 0x00, 0x01,
            ],
            match md {
                IsobmffMetadata::Audio(md) => create_mp4a(md),
                IsobmffMetadata::Video(md) => create_avc1(md),
            },
        ],
    )
}

/// Creates an `avcc` ISOBMFF box.
fn create_avcc(
    md: &VideoMetadata,
    sequence_parameter_sets: Vec<u8>,
    picture_parameter_sets: Vec<u8>,
) -> Vec<u8> {
    let props = &md.nal_video_properties;
    let mut content = vec![
        0x01,                          // configurationVersion
        props.profile_idc(),           // AVCProfileIndication
        props.profile_compatibility(), // profile_compatibility
        props.level_idc(),             // AVCLevelIndication
        0xff,                          // lengthSizeMinusOne, hard-coded to 4 bytes
        md.spss.len() as u8,
    ];
    content.extend(sequence_parameter_sets);
    content.push(md.ppss.len() as u8);
    content.extend(picture_parameter_sets);
    create_box(AVCC, &[content])
}

/// Creates an `avc1` ISOBMFF box.
fn create_avc1(md: &VideoMetadata) -> Vec<u8> {
    let mut sequence_parameter_sets: Vec<u8> = vec![];
    let mut picture_parameter_sets: Vec<u8> = vec![];
    let spss = &md.spss;
    let ppss = &md.ppss;
    let props = &md.nal_video_properties;

    // assemble the SPSs
    for sps in spss {
        sequence_parameter_sets.push(((sps.len() & 0xff00) >> 8) as u8);
        sequence_parameter_sets.push((sps.len() & 0xff) as u8); // sequenceParameterSetLength
        sequence_parameter_sets.extend(sps); // SPS
    }

    // assemble the PPSs
    for pps in ppss {
        picture_parameter_sets.push(((pps.len() & 0xff00) >> 8) as u8);
        picture_parameter_sets.push((pps.len() & 0xff) as u8);
        picture_parameter_sets.extend(pps);
    }

    let mut avc1_box: Vec<Vec<u8>> = vec![
        vec![
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // reserved
            0x00,
            0x01, // data_reference_index
            0x00,
            0x00, // pre_defined
            0x00,
            0x00, // reserved
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // pre_defined
            ((props.width() & 0xff00) >> 8) as u8,
            (props.width() & 0xff) as u8, // width
            ((props.height() & 0xff00) >> 8) as u8,
            (props.height() & 0xff) as u8, // height
            0x00,
            0x48,
            0x00,
            0x00, // horizresolution
            0x00,
            0x48,
            0x00,
            0x00, // vertresolution
            0x00,
            0x00,
            0x00,
            0x00, // reserved
            0x00,
            0x01, // frame_count
            0x13,
            0x76,
            0x69,
            0x64,
            0x65,
            0x6f,
            0x6a,
            0x73,
            0x2d,
            0x63,
            0x6f,
            0x6e,
            0x74,
            0x72,
            0x69,
            0x62,
            0x2d,
            0x68,
            0x6c,
            0x73,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // compressorname
            0x00,
            0x18, // depth = 24
            0x11,
            0x11, // pre_defined = -1
        ],
        create_avcc(md, sequence_parameter_sets, picture_parameter_sets),
        create_box(
            BTRT,
            &[vec![
                0x00, 0x1c, 0x9c, 0x80, // bufferSizeDB
                0x00, 0x2d, 0xc6, 0xc0, // maxBitrate
                0x00, 0x2d, 0xc6, 0xc0, // avgBitrate
            ]],
        ),
    ];

    let (h_spacing, v_spacing) = props.sar_ratio();
    let pasp_box: Vec<u8> = create_box(
        PASP,
        &[vec![
            0,
            0,
            ((h_spacing & 0xff00) >> 8) as u8,
            (h_spacing & 0xff) as u8,
            0,
            0,
            ((v_spacing & 0xff00) >> 8) as u8,
            (v_spacing & 0xff) as u8,
        ]],
    );
    avc1_box.push(pasp_box);
    create_box(AVC1, &avc1_box)
}

/// Creates a `mp4a` ISOBMFF box.
fn create_mp4a(md: &AudioMetadata) -> Vec<u8> {
    create_box(
        MP4A,
        &[
            vec![
                // SampleEntry, ISO/IEC 14496-12
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00, // reserved
                0x00,
                0x01, // data_reference_index
                // AudioSampleEntry, ISO/IEC 14496-12
                0x00,
                0x00,
                0x00,
                0x00, // reserved
                0x00,
                0x00,
                0x00,
                0x00, // reserved
                ((md.channel_count & 0xff00) >> 8) as u8,
                (md.channel_count & 0xff) as u8, // channelcount
                ((md.sample_size & 0xff00) >> 8) as u8,
                (md.sample_size & 0xff) as u8, // samplesize
                0x00,
                0x00, // pre_defined
                0x00,
                0x00, // reserved
                ((md.sample_rate & 0xff00) >> 8) as u8,
                (md.sample_rate & 0xff) as u8,
                0x00,
                0x00, // samplerate, 16.16

                      // MP4AudioSampleEntry, ISO/IEC 14496-14
            ],
            create_esds(
                md.audio_object_type,
                md.sampling_frequency_index,
                md.channel_count,
            ),
        ],
    )
}

/// Creates a `tkhd` ISOBMFF box.
fn create_tkhd(track_id: u32, duration: u32, width: u32, height: u32) -> Vec<u8> {
    create_box(
        TKHD,
        &[vec![
            0x00, // version 0
            0x00,
            0x00,
            0x07, // flags
            0x00,
            0x00,
            0x00,
            0x00, // creation_time
            0x00,
            0x00,
            0x00,
            0x00, // modification_time
            ((track_id & 0xff000000) >> 24) as u8,
            ((track_id & 0xff0000) >> 16) as u8,
            ((track_id & 0xff00) >> 8) as u8,
            (track_id & 0xff) as u8, // track_ID
            0x00,
            0x00,
            0x00,
            0x00, // reserved
            ((duration & 0xff000000) >> 24) as u8,
            ((duration & 0xff0000) >> 16) as u8,
            ((duration & 0xff00) >> 8) as u8,
            (duration & 0xff) as u8, // duration
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00, // reserved
            0x00,
            0x00, // layer
            0x00,
            0x00, // alternate_group
            0x01,
            0x00, // non-audio track_info volume
            0x00,
            0x00, // reserved
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x01,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x40,
            0x00,
            0x00,
            0x00, // transformation: unity matrix
            ((width & 0xff00) >> 8) as u8,
            (width & 0xff) as u8,
            0x00,
            0x00, // width
            ((height & 0xff00) >> 8) as u8,
            (height & 0xff) as u8,
            0x00,
            0x00, // height
        ]],
    )
}

/// Creates a `traf` ISOBMFF box.
fn create_traf(track_info: &TrackInfo) -> Vec<u8> {
    let track_id = track_info.track_id;
    let track_fragment_header = create_box(
        TFHD,
        &[vec![
            0x00, // version 0
            0x00,
            0x00,
            0x3a, // flags
            ((track_id & 0xff000000) >> 24) as u8,
            ((track_id & 0xff0000) >> 16) as u8,
            ((track_id & 0xff00) >> 8) as u8,
            (track_id & 0xff) as u8, // track_ID
            0x00,
            0x00,
            0x00,
            0x01, // sample_description_index
            0x00,
            0x00,
            0x00,
            0x00, // default_sample_duration
            0x00,
            0x00,
            0x00,
            0x00, // default_sample_size
            0x00,
            0x00,
            0x00,
            0x00, // default_sample_flags
        ]],
    );

    let upper_word_base_media_decode_time = track_info.base_media_decode_time / u32::MAX;
    let lower_word_base_media_decode_time = track_info.base_media_decode_time % u32::MAX;
    let track_fragment_decode_time = create_box(
        TFDT,
        &[vec![
            0x01, // version 1
            0x00,
            0x00,
            0x00, // flags
            // base_media_decode_time
            ((upper_word_base_media_decode_time >> 24) & 0xff) as u8,
            ((upper_word_base_media_decode_time >> 16) & 0xff) as u8,
            ((upper_word_base_media_decode_time >> 8) & 0xff) as u8,
            (upper_word_base_media_decode_time & 0xff) as u8,
            ((lower_word_base_media_decode_time >> 24) & 0xff) as u8,
            ((lower_word_base_media_decode_time >> 16) & 0xff) as u8,
            ((lower_word_base_media_decode_time >> 8) & 0xff) as u8,
            (lower_word_base_media_decode_time & 0xff) as u8,
        ]],
    );

    // the data offset specifies the number of bytes from the start of
    // the containing moof to the first payload byte of the associated
    // mdat
    let data_offset = 32 + // tfhd
    20 + // tfdt
    8 + // traf header
    16 + // mfhd
    8 + // moof header
    8; // mdat header

    match track_info.md {
        IsobmffMetadata::Audio(_) => {
            // audio tracks require less metadata
            let track_fragment_run =
                create_trun(MediaType::Audio, &track_info.samples, data_offset);
            create_box(
                TRAF,
                &[
                    track_fragment_header,
                    track_fragment_decode_time,
                    track_fragment_run,
                ],
            )
        }
        IsobmffMetadata::Video(_) => {
            // video tracks should contain an independent and disposable samples
            // box (sdtp)
            // generate one and adjust offsets to match
            let sample_dependency_table = create_sdtp(&track_info.samples);
            let track_fragment_run = create_trun(
                MediaType::Video,
                &track_info.samples,
                (sample_dependency_table.len() as u32) + data_offset,
            );
            create_box(
                TRAF,
                &[
                    track_fragment_header,
                    track_fragment_decode_time,
                    track_fragment_run,
                    sample_dependency_table,
                ],
            )
        }
    }
}

/// Creates a `trak` ISOBMFF box.
fn create_trak(
    md: &IsobmffMetadata,
    track_id: u32,
    duration: Option<u32>,
    sample_rate: Option<u32>,
) -> Vec<u8> {
    let duration = duration.unwrap_or(0xffffffff);
    let (width, height) = match md {
        IsobmffMetadata::Video(info) => (
            info.nal_video_properties.width(),
            info.nal_video_properties.height(),
        ),
        IsobmffMetadata::Audio(_) => (0, 0),
    };
    create_box(
        TRAK,
        &[
            create_tkhd(track_id, duration, width, height),
            create_mdia(md, duration, sample_rate),
        ],
    )
}

/// Creates a `trex` ISOBMFF box.
fn create_trex(media_type: MediaType, track_id: u32) -> Vec<u8> {
    let mut result = vec![
        0x00, // version 0
        0x00,
        0x00,
        0x00, // flags
        ((track_id & 0xff000000) >> 24) as u8,
        ((track_id & 0xff0000) >> 16) as u8,
        ((track_id & 0xff00) >> 8) as u8,
        (track_id & 0xff) as u8, // track_ID
        0x00,
        0x00,
        0x00,
        0x01, // default_sample_description_index
        0x00,
        0x00,
        0x00,
        0x00, // default_sample_duration
        0x00,
        0x00,
        0x00,
        0x00, // default_sample_size
        0x00,
        0x01,
        0x00,
        0x01, // default_sample_flags
    ];
    // the last two bytes of default_sample_flags is the sample
    // degradation priority, a hint about the importance of this sample
    // relative to others. Lower the degradation priority for all sample
    // other than video.
    if media_type != MediaType::Video {
        let last_idx = result.len() - 1;
        result[last_idx] = 0x00;
    }
    create_box(TREX, &[result])
}

pub(super) struct SampleFlag {
    is_leading: u8,
    depends_on: u8,
    is_depended_on: u8,
    has_redundancy: u8,
    is_non_sync_sample: u8,
}

pub(super) struct SampleInfo {
    data_offset: u32,
    duration: u32,
    size: u32,
    flags: SampleFlag,
    composition_time_offset: Option<u32>,
}

/// Default sample object
/// see ISO/IEC 14496-12:2012, section 8.6.4.3
pub(super) fn create_default_sample() -> SampleInfo {
    SampleInfo {
        data_offset: 0,
        size: 0,
        duration: 0,
        composition_time_offset: None,
        flags: SampleFlag {
            is_leading: 0,
            depends_on: 1,
            is_depended_on: 0,
            has_redundancy: 0,
            is_non_sync_sample: 1,
        },
    }
}

/// generate the track's sample table from an array of gops
pub(super) fn generate_sample_table(gops: GopsSet, base_data_offset: u32) -> Vec<SampleInfo> {
    let mut data_offset = base_data_offset;
    gops.gops()
        .iter()
        .flat_map(|g| g.frames())
        .map(|f| {
            let sample = sample_for_frame(f, data_offset);
            data_offset += sample.size;
            sample
        })
        .collect()
}

/// Collates information from a video frame into an object for eventual
/// entry into an MP4 sample table.
pub(super) fn sample_for_frame(frame: &FrameObject, data_offset: u32) -> SampleInfo {
    let mut sample = create_default_sample();

    sample.data_offset = data_offset;
    sample.composition_time_offset = Some(frame.pts() - frame.dts());
    sample.duration = frame.duration();
    sample.size = 4 * frame.data().len() as u32; // Space for nal unit size
    sample.size += frame.nb_bytes();

    if frame.key_frame() {
        sample.flags.depends_on = 2;
        sample.flags.is_non_sync_sample = 0;
    }
    sample
}

/// generate the track's sample table from a frame
pub(super) fn generate_sample_table_for_frame(
    frame: &FrameObject,
    base_data_offset: u32,
) -> Vec<SampleInfo> {
    vec![sample_for_frame(frame, base_data_offset)]
}

/// Creates the header of a `trun` ISOBMFF box.
///
/// This method assumes all samples are uniform. That is, if a
/// duration is present for the first sample, it will be present for
/// all subsequent samples.
fn create_trun_header(samples: &[SampleInfo], offset: u32) -> Vec<u8> {
    let mut presence_flags: u8 = 0;
    if !samples.is_empty() {
        presence_flags |= 0x1;
        presence_flags |= 0x2;
        presence_flags |= 0x4;
        if samples[0].composition_time_offset.is_some() {
            presence_flags |= 0x8;
        }
    }
    vec![
        0x00, // version 0
        0x00,
        presence_flags,
        0x01, // flags
        ((samples.len() & 0xff000000) >> 24) as u8,
        ((samples.len() & 0xff0000) >> 16) as u8,
        ((samples.len() & 0xff00) >> 8) as u8,
        (samples.len() & 0xff) as u8, // sample_count
        ((offset & 0xff000000) >> 24) as u8,
        ((offset & 0xff0000) >> 16) as u8,
        ((offset & 0xff00) >> 8) as u8,
        (offset & 0xff) as u8, // data_offset
    ]
}

/// Creates a `trun` ISOBMFF box for a video media.
fn create_video_trun(samples: &[SampleInfo], initial_offset: u32) -> Vec<u8> {
    let offset = initial_offset + 8 + 12 + 16 * samples.len() as u32;
    let header = create_trun_header(samples, offset);
    let mut bytes = Vec::with_capacity(header.len() + samples.len() * 16);
    let mut bytes_offset = header.len();
    bytes.extend(header);
    for sample in samples {
        bytes[bytes_offset] = ((sample.duration & 0xff000000) >> 24) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.duration & 0xff0000) >> 16) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.duration & 0xff00) >> 8) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.duration & 0xff) as u8; // sample_duration
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff000000) >> 24) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff0000) >> 16) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff00) >> 8) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.size & 0xff) as u8; // sample_size
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.flags.is_leading << 2) | sample.flags.depends_on;
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.flags.is_depended_on << 6)
            | (sample.flags.has_redundancy << 4)
            | sample.flags.is_non_sync_sample;
        bytes_offset += 3; // Skip degradation priority
        let composition_time_offset = if let Some(offset) = sample.composition_time_offset {
            offset
        } else {
            0
        };
        bytes[bytes_offset] = ((composition_time_offset & 0xff000000) >> 24) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((composition_time_offset & 0xff0000) >> 16) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((composition_time_offset & 0xff00) >> 8) as u8;
        bytes_offset += 1;

        // sample_composition_time_offset
        bytes[bytes_offset] = (composition_time_offset & 0xff) as u8;
        bytes_offset += 1;
    }
    create_box(TRUN, &[bytes])
}

/// Creates a `trun` ISOBMFF box for an audio media.
fn create_audio_trun(samples: &[SampleInfo], initial_offset: u32) -> Vec<u8> {
    let offset = initial_offset + 8 + 12 + 8 * samples.len() as u32;
    let header = create_trun_header(samples, offset);
    let mut bytes = Vec::with_capacity(header.len() + samples.len() * 8);
    let mut bytes_offset = header.len();
    bytes.extend(header);
    for sample in samples {
        bytes[bytes_offset] = ((sample.duration & 0xff000000) >> 24) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.duration & 0xff0000) >> 16) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.duration & 0xff00) >> 8) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.duration & 0xff) as u8; // sample_duration
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff000000) >> 24) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff0000) >> 16) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = ((sample.size & 0xff00) >> 8) as u8;
        bytes_offset += 1;
        bytes[bytes_offset] = (sample.size & 0xff) as u8; // sample_size
        bytes_offset += 1;
    }
    create_box(TRUN, &[bytes])
}

/// Creates a `trun` ISOBMFF box.
fn create_trun(media_type: MediaType, samples: &[SampleInfo], initial_offset: u32) -> Vec<u8> {
    if media_type == MediaType::Audio {
        create_audio_trun(samples, initial_offset)
    } else {
        create_video_trun(samples, initial_offset)
    }
}

/// Creates an fmp4 initialization segment for segments with the corresponding
/// track information.
pub(super) fn create_init_segment(tracks: &[TrackInfo]) -> Vec<u8> {
    let ftyp_box = create_ftyp();
    let moov_box = create_moov(tracks);
    let mut result = Vec::with_capacity(ftyp_box.len() + moov_box.len());
    result.extend(ftyp_box);
    result.extend(moov_box);
    result
}
