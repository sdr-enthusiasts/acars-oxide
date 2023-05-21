use decoders::acars::AssembledACARSMessage;
use tokio::sync::mpsc::UnboundedSender;

#[macro_use]
extern crate log;

pub mod decoders {
    pub mod acars;
}

#[derive(Debug, Clone)]
pub enum ValidDecoderType {
    ACARS,
    VDL2,
}

pub trait Decoder: Send + Sync {
    fn decode(&mut self, length: u32);
    fn get_wf_at_index(&self, index: usize) -> num::Complex<f32>;
    fn set_dm_buffer_at_index(&mut self, index: usize, value: f32);
    fn set_output_channel(&mut self, channel: UnboundedSender<AssembledACARSMessage>);
}
