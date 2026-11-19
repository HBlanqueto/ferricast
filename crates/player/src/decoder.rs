use ferricast_core::{
    CapturedFrame, Codec, DecoderConfig, EncodedFrame, FerricastError, PixelFormat, Result,
    VideoDecoder,
};
use ferricast_decoder::{H264Decoder, Vp8Decoder, Vp9Decoder};

pub(crate) enum Decoder {
    H264(H264Decoder),
    Vp8(Box<Vp8Decoder>),
    Vp9(Box<Vp9Decoder>),
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
            Codec::Vp8 => configured(&config).map(|d| Self::Vp8(Box::new(d))),
            Codec::Vp9 => configured(&config).map(|d| Self::Vp9(Box::new(d))),
            other => Err(FerricastError::Decode(format!(
                "no decoder for {other} in this build"
            ))),
        }
    }

    pub(crate) fn reorders(&self) -> bool {
        matches!(self, Decoder::H264(_))
    }

    pub(crate) fn decode(&mut self, packet: EncodedFrame) -> Result<Option<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.decode(packet),
            Decoder::Vp8(d) => d.decode(packet),
            Decoder::Vp9(d) => d.decode(packet),
        }
    }

    pub(crate) fn flush(&mut self) -> Result<Vec<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.flush(),
            Decoder::Vp8(d) => d.flush(),
            Decoder::Vp9(d) => d.flush(),
        }
    }
}

fn configured<D: VideoDecoder + Default>(config: &DecoderConfig) -> Result<D> {
    let mut decoder = D::default();
    decoder.configure(config)?;
    Ok(decoder)
}
