use oxide_decoders::decoders::acars::AssembledACARSMessage;
use tokio::sync::mpsc::UnboundedReceiver;
#[macro_use]
extern crate log;

pub struct OxideOutput {
    output_command_line: bool,
    enable_zmq: bool,
    receiver_channel: UnboundedReceiver<AssembledACARSMessage>, // TODO: This is hard coded to a single message type. We need to make this generic.
}

impl OxideOutput {
    pub fn new(
        enable_output_command_line: bool,
        enable_output_zmq: bool,
        receiver_channel: UnboundedReceiver<AssembledACARSMessage>,
    ) -> OxideOutput {
        OxideOutput {
            output_command_line: enable_output_command_line,
            enable_zmq: enable_output_zmq,
            receiver_channel,
        }
    }

    pub async fn monitor_receiver_channel(&mut self) {
        loop {
            match self.receiver_channel.try_recv() {
                Ok(message) => {
                    if self.output_command_line {
                        info!("[{: <13}] {}", "OUT CHANNEL", message);
                    } else {
                        debug!("[{: <13}] {}", "OUT CHANNEL", message);
                    }

                    if self.enable_zmq {
                        error!("[{: <13}] ZMQ output not implemented yet", "OUT CHANNEL");
                    }
                }
                Err(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
            }
        }
    }
}
