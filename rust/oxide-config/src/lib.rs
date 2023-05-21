pub extern crate clap as clap;
extern crate custom_error;

use custom_error::custom_error;
use oxide_decoders::ValidDecoderType;
use std::num::ParseFloatError;
use std::num::ParseIntError;

use clap::Parser;

const MIN_GAIN: f32 = 0.0;
const MAX_GAIN: f32 = 60.0;

#[derive(Parser, Debug, Clone, Default)]
#[command(
    name = "ACARS Oxide",
    author,
    version,
    about,
    long_about = "ACARS Oxide is a program that allows you to receive and decode ACARS and VDLM2 messages."
)]
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

pub struct OxideInput {
    /// General Program Options
    /// Set the log level. debug, trace, info are valid options. Info is default.
    #[clap(short, long, action = clap::ArgAction::Count)]
    pub logging: u8,
    /// Output received messages to stdout. Default is false.
    ///
    /// SDR specific options.
    /// For each option, the format for the command line flag is: --sdrYoptionname where Y is an integer between 1 and 8.
    /// For example, --sdr1gain 20 --sdr2gain 20
    /// The options are: gain, ppm, biastee, mult, freq, decoding_type, and serial.
    /// Please note that using the device index, as reported by rtl_test or other tools, is not supported. The serial number must be used.
    /// Of special note, `decoding_type` indicates if the message is decoded using the VDLM2 protocol or the ACARS protocol. `acars` and `vdlm2` are valid options.
    #[clap(
        long,
        env = "OXIDE_OUTPUT_TO_CONSOLE",
        value_parser,
        default_value = "false"
    )]
    pub output_to_console: bool,

    #[clap(
        long,
        env = "OXIDE_SDR1SERIAL",
        value_parser ,
        default_value = None,
        hide = true,
        requires = "sdr1freqs",
        requires = "sdr1decoding_type",
        required = true
    )]
    pub sdr1serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR1GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR1PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR1BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR1MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR1FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR1DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr1serial")
    ]
    pub sdr1decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR2SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr2freqs",
        requires = "sdr2decoding_type",
    )]
    pub sdr2serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR2GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR2PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR2BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR2MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR2FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR2DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr2serial"
    )]
    pub sdr2decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR3SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr3freqs",
        requires = "sdr3decoding_type"
    )]
    pub sdr3serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR3GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR3PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR3BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR3MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR3FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR3DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr3serial"
    )]
    pub sdr3decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR4SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr4freqs",
        requires = "sdr4decoding_type"
    )]
    pub sdr4serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR4GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR4PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR4BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR4MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR4FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR4DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr4serial"
    )]
    pub sdr4decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR5SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr5freqs",
        requires = "sdr5decoding_type"
    )]
    pub sdr5serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR5GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR5PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR5BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR5MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR5FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR5DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr5serial"
    )]
    pub sdr5decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR6SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr6freqs",
        requires = "sdr6decoding_type"
    )]
    pub sdr6serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR6GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR6PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR6BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR6MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR6FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR6DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr6serial"
    )]
    pub sdr6decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR7SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr7freqs",
        requires = "sdr7decoding_type"
    )]
    pub sdr7serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR7GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR7PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR7BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR7MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR7FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR7DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr7serial"
    )]
    pub sdr7decoding_type: Option<ValidDecoderType>,
    #[clap(
        long,
        env = "OXIDE_SDR8SERIAL",
        value_parser,
        default_value = None,
        hide = true,
        requires = "sdr8freqs",
        requires = "sdr8decoding_type"
    )]
    pub sdr8serial: Option<String>,
    #[clap(
        long,
        env = "OXIDE_SDR8GAIN",
        value_parser = parse_sdr_gain,
        default_value = "60",
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8gain: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR8PPM",
        value_parser,
        default_value = "0",
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8ppm: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR8BIASTEE",
        value_parser,
        default_value = "false",
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8biastee: Option<bool>,
    #[clap(
        long,
        env = "OXIDE_SDR8MULT",
        value_parser = validate_mult,
        default_value = "160",
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8mult: Option<i32>,
    #[clap(
        long,
        env = "OXIDE_SDR8FREQS",
        value_parser = validate_freq,
        num_args = 1..17,
        default_value = None,
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8freqs: Option<Vec<f32>>,
    #[clap(
        long,
        env = "OXIDE_SDR8DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr8serial"
    )]
    pub sdr8decoding_type: Option<ValidDecoderType>,
}

custom_error! { OxideInputError
    ParseFloat { source: ParseFloatError } = "Error parsing float",
    ParseInt { source: ParseIntError } = "Error parsing int",
    GainRange { input: f32, min: f32, max: f32 } = "Gain {input} out of range. Should be between {min} and {max}",
    Mult { input: i32 } = "Mult {input} out of range. Should be 160 or 192.",
    DecodingType { input: String } = "Decoding type {input} is not supported. Please use one of the following: VDLM2, ACARS",
    FrequencyMinMaxRange { max_freq: String, min_freq: String, range: String } = "Range between {min_freq} and {max_freq} is {range} MHz. Should be less than or equal to 2Mhz",
    FrequencyOutsideOfAirband { freq: String } = "Frequency {freq} is outside of the airband. Should be between 108 and 137 MHz",
}

fn validate_freq(freqs_string: &str) -> Result<f32, OxideInputError> {
    let freq = freqs_string.parse::<f32>()?;
    if !(108.0..=137.0).contains(&freq) {
        Err(OxideInputError::FrequencyOutsideOfAirband {
            freq: freq.to_string(),
        })
    } else {
        Ok(freq)
    }
}

fn validate_mult(env: &str) -> Result<i32, OxideInputError> {
    let mult = env.parse::<i32>()?;
    if mult != 160 && mult != 192 {
        return Err(OxideInputError::Mult { input: mult });
    }
    Ok(mult)
}

fn validate_decoding_type(env: &str) -> Result<ValidDecoderType, OxideInputError> {
    if env.to_uppercase() == "ACARS" {
        return Ok(ValidDecoderType::ACARS);
    }

    if env.to_uppercase() == "VDLM2" {
        return Ok(ValidDecoderType::VDL2);
    }

    Err(OxideInputError::DecodingType {
        input: env.to_string(),
    })
}

fn parse_sdr_gain(env: &str) -> Result<i32, OxideInputError> {
    let gain = env.parse::<f32>()?;
    if !(MIN_GAIN..=MAX_GAIN).contains(&gain) {
        return Err(OxideInputError::GainRange {
            input: gain,
            min: MIN_GAIN,
            max: MAX_GAIN,
        });
    }
    Ok(gain as i32 * 10)
}

/// A struct encapsulating the configuration for a single SDR
pub struct SDRConfig {
    pub gain: Option<u32>,
    pub ppm: Option<i32>,
    pub bias_tee: Option<bool>,
    pub mult: Option<f32>,
    pub freq: Option<Vec<String>>,
    pub serial: Option<String>,
}

impl SDRConfig {
    pub fn new(
        gain: Option<u32>,
        ppm: Option<i32>,
        bias_tee: Option<bool>,
        mult: Option<f32>,
        freq: Option<Vec<String>>,
        serial: Option<String>,
    ) -> SDRConfig {
        SDRConfig {
            gain,
            ppm,
            bias_tee,
            mult,
            freq,
            serial,
        }
    }

    pub fn get_gain(&self) -> Option<u32> {
        self.gain
    }

    pub fn get_ppm(&self) -> Option<i32> {
        self.ppm
    }

    pub fn get_bias_tee(&self) -> Option<bool> {
        self.bias_tee
    }

    pub fn get_mult(&self) -> Option<f32> {
        self.mult
    }

    pub fn get_freq(&self) -> Option<Vec<String>> {
        self.freq.clone()
    }

    pub fn get_serial(&self) -> Option<String> {
        self.serial.clone()
    }

    /// A function to determine if the SDR has been enabled by the user
    pub fn is_empty(&self) -> bool {
        if self.serial.is_none() && self.freq.is_none() {
            return true;
        }

        false
    }
}
