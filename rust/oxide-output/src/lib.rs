use oxide_decoders::decoders::acars::AssembledACARSMessage;
use tokio::sync::mpsc::Receiver;
#[macro_use]
extern crate log;

pub struct OxideOutput {
    output_command_line: bool,
    enable_zmq: bool,
    receiver_channel: Receiver<AssembledACARSMessage>, // TODO: This is hard coded to a single message type. We need to make this generic.
}

impl OxideOutput {
    pub fn new(
        enable_output_command_line: bool,
        enable_output_zmq: bool,
        receiver_channel: Receiver<AssembledACARSMessage>,
    ) -> OxideOutput {
        OxideOutput {
            output_command_line: enable_output_command_line,
            enable_zmq: enable_output_zmq,
            receiver_channel,
        }
    }

    pub async fn monitor_receiver_channel(&mut self) {
        trace!("OxideOutput::monitor_receiver_channel() called");

        while let Some(message) = self.receiver_channel.recv().await {
            println!("yooo");
            if self.output_command_line {
                println!("{}", message);
            }
            if self.enable_zmq {
                error!("ZMQ output not implemented yet");
            }
        }
        trace!("Exiting OxideOutput::monitor_receiver_channel()");
    }
}
