use ferricast_core::{
    CapturedFrame, Codec, DecoderConfig, EncodedFrame, FerricastError, PixelFormat, Result,
    VideoDecoder,
};
use ferricast_decoder::H264Decoder;

pub(crate) enum Decoder {
    H264(H264Decoder),
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
            Codec::H264 => {
                let mut decoder = H264Decoder::new();
                decoder.configure(&config)?;
                Ok(Self::H264(decoder))
            }
            other => Err(FerricastError::Decode(format!(
                "no decoder for {other} in this build"
            ))),
        }
    }

    pub(crate) fn decode(&mut self, packet: EncodedFrame) -> Result<Option<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.decode(packet),
        }
    }

    pub(crate) fn flush(&mut self) -> Result<Vec<CapturedFrame>> {
        match self {
            Decoder::H264(d) => d.flush(),
        }
    }
}
