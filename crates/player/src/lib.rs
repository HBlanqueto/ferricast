mod clock;
mod decoder;
mod limiter;
mod pts;

use std::collections::VecDeque;
use std::path::Path;

use ferricast_core::Result;
use ferricast_demuxer::Demuxer;

pub use clock::{Clock, Due};
pub use ferricast_core::{CapturedFrame, PixelFormat, RawFrame};
pub use ferricast_demuxer::{Color, VideoStream};
pub use limiter::FrameLimiter;

use decoder::Decoder;
use pts::PtsQueue;

pub struct Player {
    demuxer: Demuxer,
    decoder: Decoder,
    pending: PtsQueue,
    ready: VecDeque<CapturedFrame>,
    drained: bool,
}

impl Player {
    pub fn open(path: &Path) -> Result<Self> {
        let demuxer = Demuxer::open(path)?;
        let decoder = open_decoder(&demuxer)?;
        Ok(Self {
            demuxer,
            decoder,
            pending: PtsQueue::default(),
            ready: VecDeque::new(),
            drained: false,
        })
    }

    pub fn video(&self) -> &VideoStream {
        self.demuxer.video()
    }

    pub fn next_frame(&mut self) -> Result<Option<CapturedFrame>> {
        loop {
            if let Some(frame) = self.ready.pop_front() {
                return Ok(Some(frame));
            }
            if self.drained {
                return Ok(None);
            }
            let Some(packet) = self.demuxer.next_frame()? else {
                self.drained = true;
                for frame in self.decoder.flush()? {
                    let frame = self.stamped(frame);
                    self.ready.push_back(frame);
                }
                continue;
            };
            let pts = packet.timestamp_us;
            let reorders = self.decoder.reorders();
            if reorders {
                self.pending.push(pts);
            }
            match self.decoder.decode(packet) {
                Ok(Some(frame)) => {
                    let frame = self.stamped(frame);
                    self.ready.push_back(frame);
                }
                Ok(None) => {}
                Err(error) => {
                    if reorders {
                        self.pending.remove(pts);
                    }
                    return Err(error);
                }
            }
        }
    }

    pub fn rewind(&mut self) -> Result<()> {
        self.demuxer.rewind()?;
        self.decoder = open_decoder(&self.demuxer)?;
        self.pending.clear();
        self.ready.clear();
        self.drained = false;
        Ok(())
    }

    fn stamped(&mut self, mut frame: CapturedFrame) -> CapturedFrame {
        if !self.decoder.reorders() {
            return frame;
        }
        if let Some(pts) = self.pending.pop_earliest() {
            match &mut frame {
                CapturedFrame::Cpu(raw) => raw.timestamp_us = pts,
                CapturedFrame::Gpu(gpu) => gpu.timestamp_us = pts,
            }
        }
        frame
    }
}

fn open_decoder(demuxer: &Demuxer) -> Result<Decoder> {
    let stream = demuxer.video();
    Decoder::open(stream.codec, stream.width, stream.height)
}
