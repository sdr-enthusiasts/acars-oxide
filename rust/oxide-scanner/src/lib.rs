// Copyright (C) 2023 Fred Clausen

// This program is free software; you can redistribute it and/or
// modify it under the terms of the GNU General Public License
// as published by the Free Software Foundation; either version 2
// of the License, or (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program; if not, write to the Free Software
// Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301, USA

#[macro_use]
extern crate log;

use oxide_output::OxideOutput;
use oxide_rtlsdr::RtlSdr;
use tokio::sync::mpsc;

pub struct OxideScanner {
    sdrs: [RtlSdr; 8],
    enable_output_command_line: bool,
    enable_output_zmq: bool,
    number_of_sdrs: usize,
}

impl OxideScanner {
    pub fn new(
        sdrs: [RtlSdr; 8],
        number_of_sdrs: usize,
        enable_output_command_line: bool,
        enable_output_zmq: bool,
    ) -> OxideScanner {
        OxideScanner {
            sdrs,
            number_of_sdrs,
            enable_output_command_line,
            enable_output_zmq,
        }
    }

    pub async fn run(self) {
        let mut valid_sdrs: u8 = 0;
        let (tx_channel, rx) = mpsc::unbounded_channel();
        let mut output =
            OxideOutput::new(self.enable_output_command_line, self.enable_output_zmq, rx);

        tokio::spawn(async move {
            output.monitor_receiver_channel().await;
        });

        for mut sdr in self.sdrs.into_iter().take(self.number_of_sdrs) {
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
    }
}
