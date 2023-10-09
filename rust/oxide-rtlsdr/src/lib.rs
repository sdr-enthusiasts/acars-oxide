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
// use num_complex::Complex;
use num::Complex;
use oxide_decoders::decoders::acars::ACARSDecoder;
use oxide_decoders::decoders::acars::{self, AssembledACARSMessage};
use oxide_decoders::{Decoder, ValidDecoderType};
use rtlsdr_mt::{Controller, Reader};
use tokio::sync::mpsc::UnboundedSender;

extern crate libc;

use custom_error::custom_error;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};

// TODO: Can I wrap the librtlsdr logging functions to use the log crate?

custom_error! {pub RTLSDRError
    DeviceNotFound { sdr: String } = "Device {sdr} not found",
    FrequencySpreadTooLarge { sdr: String } = "Frequency spread too large for device {sdr}. Must be less than 2mhz",
    NoFrequencyProvided { sdr: String } = "No frequency provided for device {sdr}",
}

pub struct RtlSdr {
    ctl: Option<Controller>,
    reader: Option<Reader>,
    index: Option<u32>,
    serial: String,
    ppm: i32,
    gain: i32,
    bias_tee: bool,
    rtl_mult: i32,
    frequencies: Vec<f32>,
    channel: [Box<dyn Decoder>; 16],
    decoder_type: ValidDecoderType,
}

impl RtlSdr {
    pub fn new(
        serial: String,
        ppm: i32,
        gain: i32,
        bias_tee: bool,
        rtl_mult: i32,
        mut frequencies: Vec<f32>,
        decoder: ValidDecoderType,
    ) -> RtlSdr {
        frequencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut channels: Vec<Box<dyn Decoder>> = Vec::new();

        // FIXME: This feels so wasteful to create 16 decoders when we only need 1 or 2
        // But the array needs to be filled. Can we do better?
        for _ in 0..16_usize {
            channels.push(Box::new(ACARSDecoder::new(
                0,
                0,
                [Complex::new(0.0, 0.0); 192],
            )));
        }

        Self {
            ctl: None,
            reader: None,
            index: None,
            serial,
            ppm,
            gain,
            bias_tee,
            rtl_mult,
            frequencies,
            // array_init::array_init(|i: usize| (i * i) as u32);
            channel: array_init::from_iter(channels).unwrap(),
            decoder_type: decoder,
        }
    }

    fn get_intrate(&self) -> i32 {
        match self.decoder_type {
            ValidDecoderType::ACARS => acars::INTRATE as i32,
            ValidDecoderType::VDL2 => 0,
            ValidDecoderType::HFDL => 0,
        }
    }

    fn get_rtloutbufsz(&self) -> usize {
        match self.decoder_type {
            ValidDecoderType::ACARS => acars::RTLOUTBUFSZ,
            ValidDecoderType::VDL2 => 0,
            ValidDecoderType::HFDL => 0,
        }
    }

    fn init_channels(
        &mut self,
        output_channel: UnboundedSender<AssembledACARSMessage>,
        rtl_in_rate: i32,
    ) -> Result<i32, RTLSDRError> {
        let mut channels: Vec<i32> = Vec::new();

        for freq in self.frequencies.iter() {
            let channel = ((((1000000.0 * freq) as i32) + self.get_intrate() / 2)
                / self.get_intrate())
                * self.get_intrate();
            channels.push(channel);
        }

        // Set the center frequency. Initial implementation took the highest and lowest values
        // For the inputted frequency list and averaged them. In my mind, this would be the
        // best way to set the center frequency. However, the original acarsdec code set the center
        // using a different method. I'm not sure why, but I'm going to keep it the same for now.

        let mut center_freq_actual: i32 = channels[0];

        if channels.len() > 1 {
            let center_freq_as_float =
                ((self.frequencies[self.frequencies.len() - 1] + self.frequencies[0]) / 2.0)
                    .round();
            let center_freq = (center_freq_as_float * 1000000.0) as i32;
            info!(
                "[{: <13}] Setting center frequency to {}",
                self.serial, center_freq
            );
            center_freq_actual = center_freq;
        } else {
            info!(
                "[{: <13}] Setting center frequency to {}",
                self.serial, channels[0]
            );
        }

        let mut channel_windows = Vec::new();
        for channel in channels.iter() {
            // AMFreq = (ch->Fr - (float)Fc) / (float)(rtlInRate) * 2.0 * M_PI;
            let am_freq = ((channel - center_freq_actual) as f32) * 2.0 * std::f32::consts::PI
                / (rtl_in_rate as f32);
            let mut window: Vec<Complex<f32>> = vec![];
            for i in 0..self.rtl_mult {
                // ch->wf[ind]=cexpf(AMFreq*ind*-I)/rtlMult/127.5;
                let window_value =
                    (am_freq * i as f32 * -Complex::i()).exp() / self.rtl_mult as f32 / 127.5;
                window.push(window_value);
            }
            channel_windows.push(window);
        }

        for i in 0..self.frequencies.len() {
            // create an array out of the channel_window[i] vector
            let mut window_array: [Complex<f32>; 192] = [Complex::new(0.0, 0.0); 192];
            for (ind, window_value) in channel_windows[i].iter().enumerate() {
                window_array[ind] = *window_value;
            }
            let mut out_channel: ACARSDecoder =
                ACARSDecoder::new(i as i32, channels[i], window_array);
            out_channel.set_output_channel(output_channel.clone());

            self.channel[i] = Box::new(out_channel);
        }

        Ok(center_freq_actual)
    }

    pub fn open_sdr(
        &mut self,
        output_channel: UnboundedSender<AssembledACARSMessage>,
    ) -> Result<(), RTLSDRError> {
        let mut device_index = None;
        for dev in devices() {
            if dev.serial() == self.serial {
                device_index = Some(dev.index());
            }
        }
        match device_index {
            None => {
                return Err(RTLSDRError::DeviceNotFound {
                    sdr: self.serial.clone(),
                });
            }
            Some(idx) => {
                self.index = Some(idx);
                let rtl_in_rate = self.get_intrate() * self.rtl_mult;
                info!("[{: <13}] Using found device at index {}", self.serial, idx);

                let (mut ctl, reader) = rtlsdr_mt::open(self.index.unwrap()).unwrap();

                self.reader = Some(reader);

                // remove any duplicate frequencies
                // I cannot imagine we would EVER see this, but just in case

                self.frequencies.dedup();
                if self.frequencies.is_empty() {
                    return Err(RTLSDRError::NoFrequencyProvided {
                        sdr: self.serial.clone(),
                    });
                }

                if self.gain <= 500 {
                    let mut gains = [0i32; 32];
                    ctl.tuner_gains(&mut gains);
                    debug!("[{: <13}] Using Gains: {:?}", self.serial, gains);
                    let mut close_gain = gains[0];
                    // loop through gains and see which value is closest to the desired gain
                    for gain_value in gains {
                        if gain_value == 0 {
                            continue;
                        }

                        let err1 = i32::abs(self.gain - close_gain);
                        let err2 = i32::abs(self.gain - gain_value);

                        if err2 < err1 {
                            trace!("[{: <13}] Found closer gain: {}", self.serial, gain_value);
                            close_gain = gain_value;
                        }
                    }

                    if self.gain != close_gain {
                        warn!(
                            "[{: <13}] Input gain {} was normalized to a SDR supported gain of {}. Gain is set to the normalized gain.",
                            self.serial, self.gain, close_gain
                        );
                        self.gain = close_gain;
                    } else {
                        info!("[{: <13}] setting gain to {}", self.serial, self.gain);
                    }

                    ctl.disable_agc().unwrap();
                    ctl.set_tuner_gain(self.gain).unwrap();
                } else {
                    info!(
                        "[{: <13}] Setting gain to Auto Gain Control (AGC)",
                        self.serial
                    );
                    ctl.enable_agc().unwrap();
                }

                info!("[{: <13}] Setting PPM to {}", self.serial, self.ppm);
                ctl.set_ppm(self.ppm).unwrap();

                if self.bias_tee {
                    warn!(
                        "[{: <13}] BiasTee is not supported right now. Maybe soon...",
                        self.serial
                    );
                }
                // Verify freq spread less than 2mhz. This is much less complex than acarsdec
                // but I fail to see how this is not equivalent with a lot less bullshit

                if self.frequencies.len() > 1
                    && self.frequencies[self.frequencies.len() - 1] - self.frequencies[0] > 2.0
                {
                    return Err(RTLSDRError::FrequencySpreadTooLarge {
                        sdr: self.serial.clone(),
                    });
                }

                match self.init_channels(output_channel, rtl_in_rate) {
                    Ok(center_freq) => {
                        ctl.set_center_freq(center_freq as u32).unwrap();
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }

                info!(
                    "[{: <13}] Setting sample rate to {}",
                    self.serial, rtl_in_rate
                );
                ctl.set_sample_rate(rtl_in_rate as u32).unwrap();

                self.ctl = Some(ctl);
            }
        };

        Ok(())
    }

    pub fn close_sdr(self) {
        match self.ctl {
            None => {
                error!("[{: <13}] Device not open", self.serial);
            }
            Some(mut ctl) => {
                ctl.cancel_async_read();
            }
        }
    }

    // TODO: This function is a duplicate of the callback in read_samples. This should be refactored

    pub fn process_bytes(&mut self, bytes: &[u8], rtloutbufz: usize, vb: &mut [Complex<f32>]) {
        let mut bytes_iterator = bytes.iter();

        for m in 0..rtloutbufz {
            for vb_item in vb.iter_mut().take(self.rtl_mult as usize) {
                *vb_item = (bytes_iterator.next().expect("Ran out of bytes!").to_owned() as f32
                    - 127.37)
                    + (bytes_iterator.next().expect("Ran out of bytes!").to_owned() as f32
                        - 127.37)
                        * Complex::i();
            }

            for channel in &mut self.channel.iter_mut().take(self.frequencies.len()) {
                let mut d: Complex<f32> = Complex::new(0.0, 0.0);

                for (wf, vb_item) in vb
                    .iter()
                    .zip(channel.get_wf_iter())
                    .take(self.rtl_mult as usize)
                {
                    d += vb_item * wf;
                }

                channel.set_dm_buffer_at_index(m, d.norm());
            }
        }
        for channel in &mut self.channel.iter_mut().take(self.frequencies.len()) {
            channel.decode(rtloutbufz);
        }
    }

    pub async fn read_samples(mut self) {
        let rtloutbufz = self.get_rtloutbufsz();
        let buffer_len: u32 = rtloutbufz as u32 * self.rtl_mult as u32 * 2;
        let mut vb: [Complex<f32>; 320] = [Complex::new(0.0, 0.0); 320];

        match self.reader {
            None => {
                error!("[{: <13}] Device not open", self.serial);
            }

            Some(mut reader) => {
                reader
                    .read_async(4, buffer_len, |bytes: &[u8]| {
                        let mut bytes_iterator = bytes.iter();

                        for m in 0..rtloutbufz {
                            for vb_item in vb.iter_mut().take(self.rtl_mult as usize) {
                                *vb_item = (*bytes_iterator.next().expect("Ran out of bytes!")
                                    as f32
                                    - 127.37_f32)
                                    + (*bytes_iterator.next().expect("Ran out of bytes!") as f32
                                        - 127.37_f32)
                                        * Complex::i();
                            }

                            for channel in &mut self.channel.iter_mut().take(self.frequencies.len())
                            {
                                let mut d: Complex<f32> = Complex::new(0.0, 0.0);

                                for (wf, vb_item) in vb
                                    .iter()
                                    .zip(channel.get_wf_iter())
                                    .take(self.rtl_mult as usize)
                                {
                                    d += vb_item * wf;
                                }

                                channel.set_dm_buffer_at_index(m, d.norm());
                            }
                        }
                        for channel in &mut self.channel.iter_mut().take(self.frequencies.len()) {
                            channel.decode(rtloutbufz);
                        }
                    })
                    .unwrap();
            }
        }
    }

    pub fn get_serial(&self) -> &str {
        &self.serial
    }
}

#[derive(Debug)]
pub struct DeviceAttributes {
    vendor: String,
    product: String,
    serial: String,
    index: u32,
}

impl DeviceAttributes {
    fn new(index: u32, vendor: String, product: String, serial: String) -> Self {
        Self {
            vendor,
            product,
            serial,
            index,
        }
    }

    pub fn index(&self) -> u32 {
        self.index
    }

    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    pub fn product(&self) -> &str {
        &self.product
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }
}

impl Display for DeviceAttributes {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.write_fmt(format_args!(
            "Index: {} Vendor: {}\tProduct: {}\tSerial: {}",
            self.index, self.vendor, self.product, self.serial
        ))
    }
}

/// Create an iterator over available RTL-SDR devices.
///
/// The iterator yields a DeviceAttributes in index order, so the device with the first yielded
/// name can be opened at index 0, and so on.

pub fn devices() -> impl Iterator<Item = DeviceAttributes> {
    let count = unsafe { rtlsdr_sys::rtlsdr_get_device_count() };

    let mut devices = Vec::with_capacity(count as usize);

    for idx in 0..count {
        let mut vendor_space = [0u8; 256];
        let mut product_space = [0u8; 256];
        let mut serial_space = [0u8; 256];
        let vendor: *mut c_char = vendor_space.as_mut_ptr() as *mut c_char;
        let product: *mut c_char = product_space.as_mut_ptr() as *mut c_char;
        let serial: *mut c_char = serial_space.as_mut_ptr() as *mut c_char;

        unsafe { rtlsdr_sys::rtlsdr_get_device_usb_strings(idx, vendor, product, serial) };
        let safe_vendor = unsafe { CStr::from_ptr(vendor).to_str().unwrap() };
        let safe_product = unsafe { CStr::from_ptr(product).to_str().unwrap() };
        let safe_serial = unsafe { CStr::from_ptr(serial).to_str().unwrap() };

        devices.push(DeviceAttributes::new(
            idx,
            safe_vendor.to_string(),
            safe_product.to_string(),
            safe_serial.to_string(),
        ));
    }

    debug!("[DEVICE INIT  ] Found {} RTL-SDR devices", devices.len());
    for device in devices.iter() {
        debug!("[DEVICE INIT  ] {}", device);
    }

    devices.into_iter()
}

#[cfg(test)]
mod tests {
    use acars::AckStatus::{Ack, Nack};
    use acars::DownlinkStatus::{AirToGround, GroundToAir};
    use std::{fs::File, io::Read};

    use tokio::sync::mpsc;

    use super::*;

    // TODO: Grab a sample that includes a parity error

    #[test]
    fn test_acars_samples() -> Result<(), Box<dyn std::error::Error>> {
        let ppm = 0;
        let gain = 421;
        let bias_tee = false;
        let rtl_mult = 160;
        let frequencies = [130.025, 130.45, 131.125, 131.55];
        let decoder_type = ValidDecoderType::ACARS;

        let valid_acars_messages = [
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '5', '3', '4', 'U', 'W', ' ']),
                acknowledgement: acars::AckStatus::Nack,
                label: ['Q', '0'],
                block_id: '6',
                message_number: Some(['S', '3', '3', 'A']),
                flight_id: Some(['A', 'A', '0', '5', '4', '0']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: None,
                parity_errors: 0,
                signal_level: -11.8,
                frequency: 131.55,
                downlink_status: acars::DownlinkStatus::AirToGround,
                message_number_without_sequence: Some(['S', '3', '3']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '6', '0', 'A', 'W', ' ']),
                acknowledgement: Nack,
                label: ['Q', '0'],
                block_id: '0',
                message_number: Some(['S', '5', '8', 'A']),
                flight_id: Some(['U', 'S', '1', '8', '5', '2']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: None,
                parity_errors: 0,
                signal_level: -12.8,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['S', '5', '8']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '4', '2', '4', '9', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: 'O',
                message_number: None,
                flight_id: None,
                sublabel: Some(['M', 'D']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec!['R', 'E', 'Q', 'P', 'R', 'G', 'C', '7', '4', 'C']),
                parity_errors: 0,
                signal_level: -29.8,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '9', '6', '1', 'S', 'W', ' ']),
                acknowledgement: Nack,
                label: ['R', 'A'],
                block_id: 'A',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{17}',
                message_text: Some(vec![
                    'Q', 'U', 'H', 'D', 'Q', 'I', 'T', 'O', 'O', '.', '1', 'A', 'R', 'R', ' ', 'G',
                    'A', 'T', 'E', ' ', 'I', 'N', 'F', 'O', ' ', ' ', ' ', '\r', '\n', 'P', 'R',
                    'C', ' ', 'A', 'R', 'R', 'I', 'V', 'A', 'L', ' ', 'G', 'A', 'T', 'E', ' ', '2',
                    '\r', '\n', 'N', 'E', 'X', 'T', ' ', 'F', 'L', 'I', 'G', 'H', 'T', ' ', 'I',
                    'N', 'F', 'O', 'R', 'M', 'A', 'T', 'I', 'O', 'N', '-', '\r', '\n', '*', '*',
                    ' ', 'A', 'L', 'L', ' ', 'T', 'I', 'M', 'E', 'S', ' ', 'L', 'O', 'C', 'A', 'L',
                    ' ', '*', '*', '\r', '\n', 'A', 'C', ' ', 'N', '9', '6', '1', 'S', 'W', '\r',
                    '\n', ' ', '2', ' ', ' ', ' ', '5', '0', '6', '6', ' ', 'P', 'R', 'C', '-',
                    'L', 'A', 'X', ' ', '2', '6', '1', '7', '4', '5', '\r', '\n', 'C', 'A', ' ',
                    '0', '7', '0', '7', '7', '3', ' ', 'A', 'R', 'M', 'O', 'U', 'R', ' ', ' ', ' ',
                    ' ', ' ', ' ', ' ', ' ', '\r', '\n', ' ', '2', ' ', ' ', ' ', '5', '0', '6',
                    '6', ' ', 'P', 'R', 'C', '-', 'L', 'A', 'X', ' ', '2', '6', '1', '7', '4', '5',
                    '\r', '\n', 'F', 'O', ' ', '0', '8', '2', '8', '9', '2', ' ', 'W', 'O', 'O',
                    'L', 'D', 'R', 'I', 'D', 'G', 'E', ' ', ' ', ' ', ' ', '\r', '\n', ' ', '2',
                    ' ', ' ', ' ', '5', '0',
                ]),
                parity_errors: 0,
                signal_level: -29.9,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '5', '3', '4', 'U', 'W', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '7',
                message_number: Some(['D', '7', '5', 'A']),
                flight_id: Some(['A', 'A', '0', '5', '4', '0']),
                sublabel: Some(['D', 'F']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{17}',
                message_text: Some(vec![
                    'A', '3', '8', '/', 'A', '3', '2', '1', '3', '8', ',', '1', ',', '1', '/', 'C',
                    '1', 'T', 'R', 'P', ',', '2', '3', '2', '6', '4', '4', ',', 'K', 'C', 'L', 'T',
                    ',', 'K', 'P', 'H', 'X', ',', '0', '7', ',', '8', ',', '0', '9', '8', '3', '8',
                    '/', 'C', '2', '3', '5', '7', '7', '6', '4', ',', '-', '1', '0', '4', '3', '2',
                    '3', '7', ',', '2', '6', '0', ',', '0', '1', '6', '4', '1', ',', '4', '2', '2',
                    ',', '0', '3', '5', '1', ',', '1', '/', 'C', '3', '3', '5', '7', '6', '3', '2',
                    ',', '-', '1', '0', '4', '7', '1', '5', '6', ',', '2', '5', '9', ',', '0', '1',
                    '6', '3', '8', ',', '4', '1', '7', ',', '0', '3', '5', '2', ',', '1', '/', 'C',
                    '4', '3', '5', '7', '0', '8', '1', ',', '-', '1', '0', '5', '0', '9', '4', '0',
                    ',', '2', '6', '0', ',', '0', '1', '6', '3', '5', ',', '4', '2', '0', ',', '0',
                    '3', '5', '1', ',', '1', '/', 'C', '5', '3', '5', '6', '4', '8', '5', ',', '-',
                    '1', '0', '5', '4', '7', '2', '0', ',', '2', '5', '9', ',', '0', '1', '6', '3',
                    '2', ',', '4', '1', '7', ',', '0', '3', '5', '2', ',', '1', '/', 'C',
                ]),
                parity_errors: 0,
                signal_level: -12.8,
                frequency: 131.55,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['D', '7', '5']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '6', '0', 'A', 'W', ' ']),
                acknowledgement: Ack('0'),
                label: ['_', 'd'],
                block_id: 'Y',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.5,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '9', '2', '3', 'U', 'S', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '7',
                message_number: Some(['F', '7', '6', 'A']),
                flight_id: Some(['A', 'A', '1', '0', '3', '1']),
                sublabel: Some(['M', '1']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    'P', 'O', 'S', 'N', '3', '5', '2', '8', '6', 'W', '1', '0', '8', '5', '2', '5',
                    ',', 'G', 'U', 'P', ',', '0', '0', '4', '7', '2', '9', ',', '3', '2', '0', ',',
                    'H', 'A', 'H', 'A', 'A', ',', '0', '1', '0', '9', '0', '7', ',', ',', 'M', '4',
                    '4', ',', '2', '4', '5', '5', '9', ',', '1', '3', '8', 'E', 'C', 'B', '0',
                ]),
                parity_errors: 0,
                signal_level: -27.6,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['F', '7', '6']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '9', '2', '3', 'U', 'S', ' ']),
                acknowledgement: Ack('7'),
                label: ['H', '1'],
                block_id: 'J',
                message_number: None,
                flight_id: None,
                sublabel: Some(['M', 'D']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    'R', 'E', 'Q', 'P', 'E', 'R', ',', 'P', 'R', 'F', 'E', '3', '6',
                ]),
                parity_errors: 0,
                signal_level: -30.9,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '9', '2', '3', 'U', 'S', ' ']),
                acknowledgement: Ack('J'),
                label: ['_', 'd'],
                block_id: '8',
                message_number: Some(['S', '8', '9', 'A']),
                flight_id: Some(['A', 'A', '1', '0', '3', '1']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: None,
                parity_errors: 0,
                signal_level: -30.1,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['S', '8', '9']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '9', '2', '3', 'U', 'S', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '9',
                message_number: Some(['F', '7', '7', 'A']),
                flight_id: Some(['A', 'A', '1', '0', '3', '1']),
                sublabel: Some(['M', '1']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    'R', 'E', 'S', 'R', 'E', 'Q', '/', 'A', 'K', ',', '1', '1', '5', '8', 'A', 'F',
                    '6',
                ]),
                parity_errors: 0,
                signal_level: -31.8,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['F', '7', '7']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '1', '1', '7', '6', ' ']),
                acknowledgement: Nack,
                label: ['3', '3'],
                block_id: 'L',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    'N', ' ', 'R', 'Q', 'R', 'D', ' ', ' ', '5', '2', '7', '5', 'F', 'T', ' ', 'V',
                    'G', 'A', ' ', '1', '5', '0', '\r', '\n', 'A', 'C', 'T', 'U', 'A', 'L', ' ',
                    ' ', ' ', ' ', '4', '5', '8', '7', 'F', 'T', ' ', 'V', '2', 'P', ' ', '1', '4',
                    '8', '\r', '\n', 'K', 'D', 'E', 'N', ' ', '1', '6', 'L', ' ', ' ', ' ', ' ',
                    ' ', ' ', ' ', ' ', ' ', ' ', ' ', '1', '2', '0', '0', '0', '\r', '\n', ' ',
                    '\r', '\n', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', 'F', 'L', 'P', ' ',
                    '4', '5', ' ', ' ', 'V', 'F', 'S', ' ', '1', '6', '4', '\r', '\n', 'M', 'L',
                    'D', 'W', ' ', '4', '4', '0', '9', '2', '/', 'S', ' ', ' ', ' ', ' ', ' ', 'V',
                    'R', 'F', ' ', '1', '3', '0', '\r', '\n', 'M', 'I', 'N', ' ', 'R', 'Q', 'R',
                    'D', ' ', ' ', '5', '2', '7', '5', 'F', 'T', ' ', 'V', 'G', 'A', ' ', '1', '5',
                    '0', '\r', '\n', 'A', 'C', 'T', 'U', 'A', 'L', ' ', ' ', ' ', ' ', '4', '5',
                    '8', '7', 'F', 'T', ' ', 'V', '2', 'P', ' ', '1', '4', '8', 'D', '5', '6', 'F',
                    '\r', '\n',
                ]),
                parity_errors: 0,
                signal_level: -30.8,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '1', '1', '7', '6', ' ']),
                acknowledgement: Ack('L'),
                label: ['3', '3'],
                block_id: '1',
                message_number: Some(['M', '4', '2', 'A']),
                flight_id: Some(['C', '5', '4', '8', '2', '1']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    '9', '6', ',', 'E', ',', '3', '6', '.', '1', '9', '9', ',', '*', '*', '*', '*',
                    '*', '*', '*', ',', '2', '7', '0', '0', '7', ',', '*', '*', '*', '*', '*', ',',
                    'K', 'S', 'A', 'F', ',', 'K', 'D', 'E', 'N', ',', '1', '6', 'R', ',', '1', '6',
                    'L', ',', '9', ',', '0', ',', '0', ',', ',', ',', ',', ',', '4', '3', '.', '8',
                    ',', '0', ',', '0', ',', '0', ',', ',',
                ]),
                parity_errors: 0,
                signal_level: -29.8,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['M', '4', '2']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '1', '1', '7', '6', ' ']),
                acknowledgement: Ack('1'),
                label: ['_', 'd'],
                block_id: 'M',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.3,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '4', '6', '6', 'U', 'A', ' ']),
                acknowledgement: Ack('9'),
                label: ['_', 'd'],
                block_id: 'I',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -29.9,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '4', '6', '6', 'U', 'A', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '0',
                message_number: Some(['D', '9', '6', 'D']),
                flight_id: Some(['U', 'A', '1', '5', '8', '8']),
                sublabel: Some(['D', 'F']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{17}',
                message_text: Some(vec![
                    '5', ',', '1', '2', '5', '7', ',', '1', ',', '1', ',', '1', '/', '7', 'E', '0',
                    '7', '7', '6', ',', '0', '8', '5', '1', ',', '1', '2', '5', '6', ',', '1', ',',
                    '1', ',', '0', '/', 'E', '8', '0', '7', '8', '1', ',', '0', '8', '4', '6', ',',
                    '1', '2', '5', '7', ',', '1', ',', '1', ',', '1', '/', '8', 'E', '0', '7', '7',
                    '6', ',', '0', '8', '5', '1', ',', '1', '2', '5', '7', ',', '1', ',', '1', ',',
                    '0', '/', 'E', '9', '0', '7', '8', '5', ',', '0', '8', '4', '8', ',', '1', '2',
                    '6', '5', ',', '1', ',', '1', ',', '1', '/', '9', 'E', '0', '7', '7', '9', ',',
                    '0', '8', '5', '3', ',', '1', '2', '6', '5', ',', '1', ',', '1', ',', '0', '/',
                    'E', '0', '0', '7', '8', '8', ',', '0', '8', '5', '0', ',', '1', '2', '7', '2',
                    ',', '1', ',', '1', ',', '1', '/', '0', 'E', '0', '7', '8', '3', ',', '0', '8',
                    '5', '5', ',', '1', '2', '7', '4', ',', '1', ',', '1', ',', '0', '/', 'N', '1',
                    '0', '7', '8', '9', ',', '0', '8', '5', '0', ',', '1', '2', '7', '4', ',', '1',
                    ',', '1', ',', '1', '/', '1', 'N', '0', '7', '8', '4', ',', '0', '8',
                ]),
                parity_errors: 0,
                signal_level: -22.9,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['D', '9', '6']),
                message_number_sequence: Some('D'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '5', '7', '2', 'U', 'W', ' ']),
                acknowledgement: Ack('8'),
                label: ['_', 'd'],
                block_id: 'O',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.2,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: None,
                acknowledgement: Nack,
                label: ['S', 'Q'],
                block_id: '\0',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    '0', '2', 'X', 'A', 'A', 'B', 'Q', 'K', 'A', 'B', 'Q', '1', '3', '5', '0', '2',
                    'N', '1', '0', '6', '3', '7', 'W', 'V', '1', '3', '6', '9', '7', '5', '/', 'A',
                    'R', 'I', 'N', 'C',
                ]),
                parity_errors: 0,
                signal_level: -20.2,
                frequency: 131.55,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '5', '7', '2', 'U', 'W', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '9',
                message_number: Some(['D', '5', '5', 'B']),
                flight_id: Some(['A', 'A', '2', '5', '5', '4']),
                sublabel: Some(['D', 'F']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{17}',
                message_text: Some(vec![
                    '6', '3', '6', '7', '8', '4', '9', ',', '-', '1', '0', '8', '3', '2', '3', '1',
                    ',', '2', '8', '0', ',', '0', '1', '6', '8', '2', ',', '4', '6', '0', ',', '0',
                    '3', '1', '8', ',', '1', '/', 'C', '7', '0', '0', '0', '0', '5', '8', '4', ',',
                    '0', '0', '0', '0', '0', '8', '8', ',', '0', '4', '2', '9', ',', '2', '3', '8',
                    '3', '3', ',', '2', '0', '4', '7', '8', ',', '-', '3', '2', '/', 'C', '8', '0',
                    '0', '0', '0', '1', '0', '2', ',', '0', '0', '0', '0', '0', '2', '7', ',', '0',
                    '5', '1', '2', ',', '2', '4', '3', '2', '1', ',', '2', '0', '1', '5', '8', ',',
                    '-', '3', '2', '/', 'C', '9', '0', '0', '0', '0', '0', '6', '6', ',', '0', '0',
                    '0', '0', '0', '3', '3', ',', '0', '5', '6', '1', ',', '2', '4', '4', '1', '8',
                    ',', '1', '9', '8', '3', '8', ',', '-', '3', '2', '/', 'C', '0', '0', '0', '0',
                    '0', '2', '2', '5', ',', '0', '0', '0', '0', '0', '3', '6', ',', '0', '5', '8',
                    '2', ',', '2', '3', '5', '7', '1', ',', '1', '9', '4', '7', '8', ',', '-', '3',
                    '4', '/', 'E', '1', '0', '0', '0', '0', '2', '9', '4', ',', '0', '0',
                ]),
                parity_errors: 0,
                signal_level: -24.8,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['D', '5', '5']),
                message_number_sequence: Some('B'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '1', '4', 'U', 'W', ' ']),
                acknowledgement: Nack,
                label: ['4', '0'],
                block_id: 'O',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    'F', 'L', 'T', ' ', '1', '8', '9', '8', '/', '2', '6', ' ', 'M', 'S', 'N', '-',
                    'P', 'H', 'X', '\r', '\n', '\r', '\n', 'G', 'A', 'T', 'E', ' ', 'A', '1', '7',
                    ' ', 'P', 'C', 'A', ' ', 'Y', ' ', 'E', 'L', 'E', 'C', ' ', 'Y', ' ', '1', '2',
                    '9', '.', '6', '2', ' ', '\r', '\n', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
                    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
                    ' ', '\r', '\n', 'C', 'R', 'E', 'W', ' ', 'C', 'O', 'N', 'N', 'E', 'C', 'T',
                    'I', 'N', 'G', ' ', 'G', 'A', 'T', 'E', ' ', 'I', 'N', 'F', 'O', '\r', '\n',
                    'C', 'R', 'E', 'W', ' ', 'S', 'E', 'A', 'T', ' ', 'F', 'L', 'T', ' ', 'D', 'E',
                    'S', 'T', ' ', 'G', 'A', 'T', 'E', ' ', 'T', 'I', 'M', 'E', '\r', '\n', ' ',
                    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ',
                    ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', ' ', '\r', '\n', ' ', 'N',
                    'O', ' ', 'C', 'R', 'E', 'W', ' ', 'C', 'N', 'X', '\r', '\n', 'E', 'N', 'D',
                    '\r', '\n',
                ]),
                parity_errors: 0,
                signal_level: -30.2,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '1', '4', 'U', 'W', ' ']),
                acknowledgement: Ack('O'),
                label: ['_', 'd'],
                block_id: '3',
                message_number: Some(['S', '9', '5', 'A']),
                flight_id: Some(['A', 'A', '1', '8', '9', '8']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: None,
                parity_errors: 0,
                signal_level: -11.2,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['S', '9', '5']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '5', '3', 'A', 'W', ' ']),
                acknowledgement: Ack('4'),
                label: ['_', 'd'],
                block_id: 'M',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.3,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '5', '3', 'A', 'W', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '5',
                message_number: Some(['D', '3', '4', 'B']),
                flight_id: Some(['U', 'S', '1', '9', '0', '7']),
                sublabel: Some(['D', 'F']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{17}',
                message_text: Some(vec![
                    '1', '0', '8', '1', '9', '8', '3', ',', '3', '2', '6', ',', '0', '1', '3', '5',
                    '7', ',', '4', '6', '2', ',', '0', '2', '5', '8', ',', '1', '/', 'C', '6', '3',
                    '5', '2', '5', '3', '9', ',', '-', '1', '0', '8', '5', '6', '8', '2', ',', '3',
                    '2', '0', ',', '0', '1', '3', '5', '4', ',', '4', '6', '4', ',', '0', '2', '6',
                    '7', ',', '1', '/', 'C', '7', '0', '0', '0', '0', '0', '5', '7', ',', '0', '0',
                    '0', '0', '0', '3', '8', ',', '0', '5', '5', '4', ',', '2', '5', '6', '6', '2',
                    ',', '1', '0', '7', '9', '9', ',', '-', '5', '2', '/', 'C', '8', '0', '0', '0',
                    '0', '8', '4', '0', ',', '0', '0', '0', '0', '0', '2', '0', ',', '0', '6', '6',
                    '4', ',', '2', '5', '2', '7', '8', ',', '1', '0', '6', '3', '9', ',', '-', '5',
                    '3', '/', 'C', '9', '0', '0', '0', '0', '8', '7', '6', ',', '0', '0', '0', '0',
                    '0', '7', '0', ',', '0', '5', '3', '7', ',', '2', '4', '7', '9', '6', ',', '1',
                ]),
                parity_errors: 0,
                signal_level: -20.0,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['D', '3', '4']),
                message_number_sequence: Some('B'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '1', '4', '2', '2', '8', ' ']),
                acknowledgement: Nack,
                label: ['_', 'd'],
                block_id: '4',
                message_number: Some(['S', '6', '2', 'A']),
                flight_id: Some(['U', 'A', '0', '5', '3', '9']),
                sublabel: None,
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: None,
                parity_errors: 0,
                signal_level: -31.4,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['S', '6', '2']),
                message_number_sequence: Some('A'),
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '3', '4', '1', 'F', 'R', ' ']),
                acknowledgement: Ack('0'),
                label: ['_', 'd'],
                block_id: 'R',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.2,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '5', '3', 'A', 'W', ' ']),
                acknowledgement: Ack('5'),
                label: ['_', 'd'],
                block_id: 'N',
                message_number: None,
                flight_id: None,
                sublabel: None,
                mfi: None,
                block_start: '\u{3}',
                block_end: '\u{3}',
                message_text: Some(vec!['\0']),
                parity_errors: 0,
                signal_level: -30.1,
                frequency: 130.025,
                downlink_status: GroundToAir,
                message_number_without_sequence: None,
                message_number_sequence: None,
            },
            acars::AssembledACARSMessage {
                mode: '2',
                aircraft_tail: Some(['N', '6', '5', '3', 'A', 'W', ' ']),
                acknowledgement: Nack,
                label: ['H', '1'],
                block_id: '6',
                message_number: Some(['D', '3', '4', 'C']),
                flight_id: Some(['U', 'S', '1', '9', '0', '7']),
                sublabel: Some(['D', 'F']),
                mfi: None,
                block_start: '\u{2}',
                block_end: '\u{3}',
                message_text: Some(vec![
                    '0', '2', '7', '9', ',', '-', '5', '3', '/', 'C', '0', '0', '0', '0', '0', '0',
                    '9', '0', ',', '0', '0', '0', '0', '0', '3', '8', ',', '0', '5', '6', '2', ',',
                    '2', '4', '3', '4', '4', ',', '1', '0', '0', '7', '9', ',', '-', '4', '6', '/',
                    'E', '1', '0', '0', '0', '0', '3', '5', '2', ',', '0', '0', '0', '0', '0', '1',
                    '6', ',', '0', '5', '7', '2', ',', '2', '4', '1', '9', '2', ',', '0', '9', '7',
                    '9', '9', ',', '-', '4', '4', '/', 'E', '2', '0', '0', '1', '8', '1', '6', '3',
                    ',', '0', '1', '0', '8', '2', ',', '0', '1', '0', '0', '1', ',', '0', '0', '0',
                    '0', '1', ',', '-', '0', '0', '1', '0', ',', '0', '0', ',', '2', '8', '6', '/',
                    'E', '3', '0', '0', '0', ',', '0', '1', '/',
                ]),
                parity_errors: 0,
                signal_level: -21.2,
                frequency: 130.025,
                downlink_status: AirToGround,
                message_number_without_sequence: Some(['D', '3', '4']),
                message_number_sequence: Some('C'),
            },
        ];

        let mut rtl = RtlSdr::new(
            "00000001".to_string(),
            ppm,
            gain,
            bias_tee,
            rtl_mult,
            frequencies.to_vec(),
            decoder_type,
        );

        let (tx_channel, mut rx) = mpsc::unbounded_channel();

        match rtl.init_channels(tx_channel, rtl.get_intrate() * rtl.rtl_mult) {
            Ok(_) => {
                info!("[{: <13}] Channels initialized", rtl.get_serial());
            }
            Err(e) => {
                error!(
                    "[{: <13}] Error initializing channels: {}",
                    rtl.get_serial(),
                    e
                );

                // return error
                return Err(Box::new(e));
            }
        }

        let rtloutbufz = rtl.get_rtloutbufsz();
        let buffer_len: u32 = rtloutbufz as u32 * rtl.rtl_mult as u32 * 2;
        let mut vb: [Complex<f32>; 320] = [Complex::new(0.0, 0.0); 320];

        for i in 1..=6 {
            let mut num_reads = 0;
            let mut file = File::open(format!("../../test data/acars_0{}.bin", i))?;
            loop {
                let mut buffer = vec![];
                let n = file
                    .by_ref()
                    .take(buffer_len as u64)
                    .read_to_end(&mut buffer)?;
                if n == 0 {
                    //return an error if we didn't read anything
                    break;
                }

                assert!(
                    buffer.len() == buffer_len as usize,
                    "Buffer length is not {}. Found {}",
                    buffer_len,
                    buffer.len()
                );

                num_reads += 1;

                rtl.process_bytes(&buffer, rtloutbufz, &mut vb);
            }

            assert!(
                num_reads == 100,
                "Number of reads is not 100. Found {}",
                num_reads
            );
        }

        // go through the rx channel and check that we have the correct number of messages

        let mut num_messages = 0;

        loop {
            match rx.try_recv() {
                Ok(msg) => {
                    if num_messages < valid_acars_messages.len() {
                        assert_eq!(
                            msg, valid_acars_messages[num_messages],
                            "Message {} is not equal to valid message",
                            num_messages
                        );
                    }

                    num_messages += 1;
                    println!("{}", msg);
                }
                Err(_) => break,
            }
        }

        assert!(
            num_messages == valid_acars_messages.len(),
            "Number of messages is not {}. Found {}",
            valid_acars_messages.len(),
            num_messages
        );

        Ok(())
    }
}
