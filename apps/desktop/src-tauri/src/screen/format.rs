//! The stream's format, chosen from what the tablet decodes
//! (`ScreenCapability`) and what the PC encodes.

use padboard_protocol::{ScreenCapability, VideoCodec};

/// Frames per second asked for (fewer when the screen does not change).
pub const FPS: u32 = 60;

/// What a stream carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamFormat {
    pub codec: VideoCodec,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

/// The tablet's first decoder whose codec the PC encodes (`encoders`), at
/// the largest size of the tablet's screen shape within that decoder's
/// limits: width a multiple of 16 (encoders work in 16×16 blocks), height
/// even. SM-T580 (1920×1200, H.264 up to 1920×1088): 1728×1080.
#[must_use]
pub fn choose(capability: &ScreenCapability, encoders: &[VideoCodec]) -> Option<StreamFormat> {
    let (dw, dh) = (
        u64::from(capability.display_width),
        u64::from(capability.display_height),
    );
    if dw == 0 || dh == 0 {
        return None;
    }
    capability
        .decoders
        .iter()
        .filter(|d| encoders.contains(&d.codec))
        .find_map(|d| {
            let max_w = u64::from(d.max_width).min(dw);
            let max_h = u64::from(d.max_height).min(dh);
            // Height first, then the width it gives, within both limits.
            let mut width = (max_h * dw / dh).min(max_w) / 16 * 16;
            let mut height = width * dh / dw / 2 * 2;
            while height > max_h && width >= 16 {
                width -= 16;
                height = width * dh / dw / 2 * 2;
            }
            if width < 16 || height < 2 {
                return None;
            }
            Some(StreamFormat {
                codec: d.codec,
                width: u32::try_from(width).ok()?,
                height: u32::try_from(height).ok()?,
                fps: FPS,
            })
        })
}

#[cfg(test)]
mod tests {
    use padboard_protocol::ScreenDecoder;

    use super::*;

    fn tablet(w: u32, h: u32, decoders: &[(VideoCodec, u32, u32)]) -> ScreenCapability {
        ScreenCapability {
            display_width: w,
            display_height: h,
            decoders: decoders
                .iter()
                .map(|&(codec, max_width, max_height)| ScreenDecoder {
                    codec,
                    max_width,
                    max_height,
                })
                .collect(),
            touch: true,
        }
    }

    #[test]
    fn the_sm_t580_gets_1728_by_1080() {
        let cap = tablet(
            1920,
            1200,
            &[
                (VideoCodec::H264, 1920, 1088),
                (VideoCodec::Vp8, 1920, 1088),
            ],
        );
        assert_eq!(
            choose(&cap, &[VideoCodec::H264, VideoCodec::Vp8]),
            Some(StreamFormat {
                codec: VideoCodec::H264,
                width: 1728,
                height: 1080,
                fps: FPS
            })
        );
        // A PC without H.264 (Linux without VA-API): VP8.
        assert_eq!(
            choose(&cap, &[VideoCodec::Vp8]).map(|f| f.codec),
            Some(VideoCodec::Vp8)
        );
    }

    #[test]
    fn a_small_screen_is_not_scaled_up() {
        let cap = tablet(1280, 800, &[(VideoCodec::H264, 4096, 2304)]);
        let f = choose(&cap, &[VideoCodec::H264]).unwrap();
        assert_eq!((f.width, f.height), (1280, 800));
    }

    #[test]
    fn a_wide_screen_is_limited_by_the_decoder_width() {
        let cap = tablet(2560, 1080, &[(VideoCodec::H264, 1920, 1088)]);
        let f = choose(&cap, &[VideoCodec::H264]).unwrap();
        assert!(f.width <= 1920 && f.height <= 1088, "{f:?}");
        assert_eq!(f.width % 16, 0);
        assert_eq!(f.height % 2, 0);
    }

    #[test]
    fn nothing_in_common_means_no_stream() {
        let cap = tablet(1920, 1200, &[(VideoCodec::Vp8, 1920, 1088)]);
        assert_eq!(choose(&cap, &[VideoCodec::H264]), None);
        assert_eq!(choose(&tablet(0, 0, &[]), &[VideoCodec::H264]), None);
    }
}
