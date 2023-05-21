#[macro_use]
extern crate log;

use oxide_output::OxideOutput;
use oxide_rtlsdr::RtlSdr;
use tokio::sync::mpsc;

pub struct OxideScanner {
    sdrs: Vec<RtlSdr>,
    enable_output_command_line: bool,
    enable_output_zmq: bool,
}

impl OxideScanner {
    pub fn new(
        sdrs: Vec<RtlSdr>,
        enable_output_command_line: bool,
        enable_output_zmq: bool,
    ) -> OxideScanner {
        OxideScanner {
            sdrs,
            enable_output_command_line,
            enable_output_zmq,
        }
    }

    pub async fn run(self) {
        let mut valid_sdrs: u8 = 0;
        let (tx_channel, rx) = mpsc::unbounded_channel();

        for mut sdr in self.sdrs.into_iter() {
            info!("[OXIDE SCANNER] Opening SDR {}", sdr.get_serial());
            match sdr.open_sdr(tx_channel.clone()) {
                Ok(_) => {
                    valid_sdrs += 1;
                    info!("[OXIDE SCANNER] SDR {} opened", sdr.get_serial());
                    tokio::spawn(async move {
                        sdr.read_samples().await;
                    });
                }
                Err(e) => {
                    error!(
                        "[OXIDE SCANNER] Failed to open SDR {}: {}",
                        sdr.get_serial(),
                        e
                    );
                }
            }
        }

        assert!(valid_sdrs > 0, "No valid SDRs found. Exiting program.");

        let mut output =
            OxideOutput::new(self.enable_output_command_line, self.enable_output_zmq, rx);

        tokio::spawn(async move {
            output.monitor_receiver_channel().await;
        });
    }
}
