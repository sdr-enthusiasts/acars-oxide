// Copyright (C) 2023  Fred Clausen

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
use num_complex::Complex;
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
    fn get_wf_iter(&self) -> std::slice::Iter<'_, Complex<f32>>;
    fn set_dm_buffer_at_index(&mut self, index: usize, value: f32);
    fn set_output_channel(&mut self, channel: UnboundedSender<AssembledACARSMessage>);
}
