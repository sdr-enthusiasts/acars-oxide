// Copyright (C) 2023-2024 Fred Clausen

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

#![deny(
    clippy::pedantic,
    //clippy::cargo,
    clippy::nursery,
    clippy::style,
    clippy::correctness,
    clippy::all,
    clippy::unwrap_used,
    clippy::expect_used
)]
// #![warn(missing_docs)]

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
