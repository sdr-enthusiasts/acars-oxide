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

use rtlsdr_mt::{Controller, Reader};

fn main() {
    // TODO: Make these command line switches
    let sample_rate = 2000000;
    let center_freq = 131000000;
    let rtl_mult = 160;
    let sample_size = 1024 * 160 * 2; // rtl buf z * rtl_mult * 2
    let device_index = 1;
    let gain = 421;
    let ppm = 0;

    let mut num_samples = 100;

    let (mut ctl, reader) = rtlsdr_mt::open(device_index).unwrap();
    ctl.disable_agc().unwrap();
    ctl.set_tuner_gain(gain).unwrap();
    ctl.set_center_freq(center_freq).unwrap();
    ctl.set_ppm(ppm).unwrap();

    reader.read_async(4, sample_size, |bytes| {
        while num_samples > 0 {
            num_samples -= 1;
        }
    });

    ctl.cancel_async_read.unwrap();
}
