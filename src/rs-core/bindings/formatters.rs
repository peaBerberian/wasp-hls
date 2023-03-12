use crate::parser::{VideoResolution, VariantStream, ByteRange};


static NULL_RESOLUTION: VideoResolution = VideoResolution::new(0, 0);

pub(crate) fn format_variants_info_for_js(
    variants: &[VariantStream]
) -> Vec<u32> {
    let mut ret: Vec<u32> = vec![];
    ret.push(variants.len() as u32);
    variants.iter().for_each(|v| {
        let resolution = v.resolution().unwrap_or(&NULL_RESOLUTION);
        ret.push(v.id());
        ret.push(resolution.height() as u32);
        ret.push(resolution.width() as u32);
        ret.push(v.frame_rate().unwrap_or(0.) as u32);
        ret.push(v.bandwidth as u32);
    });
    ret
}

pub(crate) fn format_range_for_js(
    original: Option<&ByteRange>
) -> (Option<usize>, Option<usize>) {
    match original {
        None => (None, None),
        Some(ByteRange { first_byte, last_byte }) => {
            (Some(*first_byte), Some(*last_byte))
        }
    }
}
