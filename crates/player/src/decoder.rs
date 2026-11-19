use ferricast_core::{
    CapturedFrame, Codec, DecoderConfig, EncodedFrame, PixelFormat, Result, VideoDecoder,
};
use ferricast_decoder::{Av1Decoder, H264Decoder, H265Decoder, Vp8Decoder, Vp9Decoder};

pub(crate) enum Decoder {
    H264(H264Decoder),
    H265(H265Decoder),
    Vp8(Box<Vp8Decoder>),
    Vp9(Box<Vp9Decoder>),
    Av1(Av1Decoder),
}

impl Decoder {
    pub(crate) fn open(codec: Codec, width: u32, height: u32) -> Result<Self> {
        let config = DecoderConfig {
            codec,
            width,
            height,
            pixel_format: PixelFormat::Nv12,
            low_latency: false,
        };
        match codec {
            Codec::H264 => configured(&config).map(Self::H264),
            Codec::H265 => configured(&config).map(Self::H265),
            Codec::Vp8 => configured(&config).map(|d| Self::Vp8(Box::new(d))),
            Codec::Vp9 => configured(&config).map(|d| Self::Vp9(Box::new(d))),
            Codec::Av1 => configured(&config).map(Self::Av1),
        }
    }

    pub(crate) fn reorders(&self) -> bool {
        matches!(self, Decoder::H264(_) | Decoder::H265(_))
    }

    pub(crate) fn decode(&mut self, packet: EncodedFrame) -> Result<Option<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.decode(packet),
            Decoder::H265(d) => d.decode(packet),
            Decoder::Vp8(d) => d.decode(packet),
            Decoder::Vp9(d) => d.decode(packet),
            Decoder::Av1(d) => d.decode(packet),
        }
    }

    pub(crate) fn flush(&mut self) -> Result<Vec<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.flush(),
            Decoder::H265(d) => d.flush(),
            Decoder::Vp8(d) => d.flush(),
            Decoder::Vp9(d) => d.flush(),
            Decoder::Av1(d) => d.flush(),
        }
    }
}

fn configured<D: VideoDecoder + Default>(config: &DecoderConfig) -> Result<D> {
    let mut decoder = D::default();
    decoder.configure(config)?;
    Ok(decoder)
}
