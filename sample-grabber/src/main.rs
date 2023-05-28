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

use std::fs::File;
use std::io::prelude::*;
pub extern crate clap as clap;
use clap::Parser;

#[derive(Parser, Debug, Clone, Default)]
#[command(
    name = "Sample Grabber",
    author,
    version,
    about,
    long_about = "Sample Grabber is a simple program that enables you to grab raw samples from an RTLSDR device."
)]

struct Input {
    #[clap(short, long, value_parser, default_value = "0")]
    device_index: u32,
    #[clap(short, long, value_parser, default_value = "131000000")]
    center_freq: u32,
    #[clap(short, long, value_parser, default_value = "421")]
    gain: i32,
    #[clap(short, long, value_parser, default_value = "0")]
    ppm: i32,
    #[clap(short, long, value_parser, default_value = "100")]
    num_samples: u32,
    #[clap(short, long, value_parser, default_value = "acars.bin")]
    output_file: String,
    #[clap(short, long, value_parser, default_value = "160")]
    rtl_mult: u32,
}

fn main() -> std::io::Result<()> {
    let args = Input::parse();
    let center_freq = args.center_freq;
    let rtl_mult = args.rtl_mult;
    let sample_size = 1024 * rtl_mult * 2; // rtl buf z * rtl_mult * 2
    let device_index = args.device_index;
    let gain = args.gain;
    let ppm = args.ppm;

    let mut num_samples = 100;

    let (mut ctl, mut reader) = rtlsdr_mt::open(device_index).unwrap();
    ctl.disable_agc().unwrap();
    ctl.set_tuner_gain(gain).unwrap();
    ctl.set_center_freq(center_freq).unwrap();
    ctl.set_ppm(ppm).unwrap();

    let mut file = File::create("acars.bin")?;
    reader
        .read_async(4, sample_size, |bytes| {
            if num_samples > 0 {
                num_samples -= 1;

                file.write_all(bytes).unwrap();
            } else {
                ctl.cancel_async_read();
            }
        })
        .unwrap();

    Ok(())
}
