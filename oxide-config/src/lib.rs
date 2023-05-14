pub extern crate clap as clap;
extern crate custom_error;

use custom_error::custom_error;
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
        requires = "sdr1decoding_type"
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
        num_args = 1..9,
        default_value = None,
        hide = true,
        requires = "sdr1serial"
    )]
    pub sdr1freqs: Option<Vec<i32>>,
    #[clap(
        long,
        env = "OXIDE_SDR1DECODING_TYPE",
        value_parser = validate_decoding_type,
        default_value = None,
        hide = true,
        requires = "sdr1serial")
    ]
    pub sdr1decoding_type: Option<String>,
}

custom_error! { OxideInputError
    ParseFloatError { source: ParseFloatError } = "Error parsing float",
    ParseIntError { source: ParseIntError } = "Error parsing int",
    GainRangeError { input: f32, min: f32, max: f32 } = "Gain {input} out of range. Should be between {min} and {max}",
    MultError { input: i32 } = "Mult {input} out of range. Should be 160 or 192.",
    DecodingTypeError { input: String } = "Decoding type {input} is not supported. Please use one of the following: VDLM2, ACARS",
    FrequencyMinMaxRangeError { max_freq: String, min_freq: String, range: String } = "Range between {min_freq} and {max_freq} is {range} MHz. Should be less than or equal to 2Mhz",
    FrequencyOutsideOfAirbandError { freq: String } = "Frequency {freq} is outside of the airband. Should be between 108 and 137 MHz",
}

fn validate_freq(freqs_string: &str) -> Result<i32, OxideInputError> {
    let freq = freqs_string.parse::<f32>()?;
    if freq < 108.0 || freq > 137.0 {
        return Err(OxideInputError::FrequencyOutsideOfAirbandError {
            freq: freq.to_string(),
        });
    } else {
        return Ok((freq * 1000000.0) as i32);
    }
}

// fn validate_freq(freqs_string: &str) -> Result<std::vec::Vec<i32>, OxideInputError> {
//     println!("Calling function {}", freqs_string);
//     let freqs: Vec<&str> = freqs_string.split("\n").collect();
//     if freqs.len() > 8 {
//         return Err(OxideInputError::TooManyFreqsError {
//             number_of_freqs: freqs.len() as i32,
//         });
//     }

//     let mut freqs_int = vec![];

//     for freq in freqs {
//         println!("test {}", freq);
//         let freq = freq.parse::<f32>()?;
//         if freq < 108.0 || freq > 137.0 {
//             return Err(OxideInputError::FrequencyOutsideOfAirbandError {
//                 freq: freq.to_string(),
//             });
//         } else {
//             freqs_int.push((freq * 1000000.0) as i32);
//         }
//     }

//     Ok(freqs_int)
// }

fn validate_mult(env: &str) -> Result<i32, OxideInputError> {
    let mult = env.parse::<i32>()?;
    if mult != 160 && mult != 192 {
        return Err(OxideInputError::MultError { input: mult });
    }
    Ok(mult)
}

fn validate_decoding_type(env: &str) -> Result<String, OxideInputError> {
    if env.to_uppercase() != "ACARS" && env.to_uppercase() != "VDLM2" {
        return Err(OxideInputError::DecodingTypeError {
            input: env.to_string(),
        });
    }
    Ok(env.to_string())
}

fn parse_sdr_gain(env: &str) -> Result<i32, OxideInputError> {
    let gain = env.parse::<f32>()?;
    if gain < MIN_GAIN || gain > MAX_GAIN {
        return Err(OxideInputError::GainRangeError {
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
            gain: gain,
            ppm: ppm,
            bias_tee: bias_tee,
            mult: mult,
            freq: freq,
            serial: serial,
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
