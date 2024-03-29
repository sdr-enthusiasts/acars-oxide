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

use decoders::acars::AssembledACARSMessage;
//use num_complex::Complex;
use num::Complex;
use tokio::sync::mpsc::UnboundedSender;

#[macro_use]
extern crate log;

pub mod decoders {
    pub mod acars;
}

/// Enum to represent the different types of decoders
#[derive(Debug, Clone)]
pub enum ValidDecoderType {
    ACARS,
    VDL2,
    HFDL,
}

/// Trait to represent a decoder.
pub trait Decoder: Send + Sync {
    /// function to pass through to the decoder implementation data read in from the SDR
    fn decode(&mut self, length: usize);
    /// function to grab the WF data iterator from the decoder implementation.
    /// Used during SDR data processing before passing the data to the decoder
    fn get_wf_iter(&self) -> std::slice::Iter<'_, Complex<f32>>;
    /// function to set the dm buffer in the decoder to a processed value from the SDR
    fn set_dm_buffer_at_index(&mut self, index: usize, value: f32);
    /// function to set the output channel for the decoder to pass processed messages to
    fn set_output_channel(&mut self, channel: UnboundedSender<AssembledACARSMessage>);
}
