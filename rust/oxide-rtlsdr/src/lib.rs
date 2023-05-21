#[macro_use]
extern crate log;
use oxide_decoders::decoders::acars::ACARSDecoder;
use oxide_decoders::decoders::acars::{self, AssembledACARSMessage};
use oxide_decoders::{Decoder, ValidDecoderType};
use rtlsdr_mt::{Controller, Reader};
use tokio::sync::mpsc::Sender;

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
    channel: Vec<Box<dyn Decoder>>,
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
            channel: vec![],
            decoder_type: decoder,
        }
    }

    fn get_intrate(&self) -> i32 {
        match self.decoder_type {
            ValidDecoderType::ACARS => acars::INTRATE,
            ValidDecoderType::VDL2 => 0,
        }
    }

    fn get_rtloutbufsz(&self) -> usize {
        match self.decoder_type {
            ValidDecoderType::ACARS => acars::RTLOUTBUFSZ,
            ValidDecoderType::VDL2 => 0,
        }
    }

    pub fn open_sdr(
        &mut self,
        output_channel: Sender<AssembledACARSMessage>,
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
                info!("[{: <13}] Using Found device at index {}", self.serial, idx);

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

                let rtl_in_rate = self.get_intrate() * self.rtl_mult;
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

                let center_freq_actual: i32;

                if channels.len() > 1 {
                    let center_freq_as_float = ((self.frequencies[self.frequencies.len() - 1]
                        + self.frequencies[0])
                        / 2.0)
                        .round();
                    let center_freq = (center_freq_as_float * 1000000.0) as i32;
                    info!(
                        "[{: <13}] Setting center frequency to {}",
                        self.serial, center_freq
                    );
                    center_freq_actual = center_freq;
                    ctl.set_center_freq(center_freq as u32).unwrap();
                } else {
                    info!(
                        "[{: <13}] Setting center frequency to {}",
                        self.serial, channels[0]
                    );
                    center_freq_actual = channels[0];
                    ctl.set_center_freq(channels[0] as u32).unwrap();
                }

                let mut channel_windows = Vec::new();
                for channel in channels.iter() {
                    // AMFreq = (ch->Fr - (float)Fc) / (float)(rtlInRate) * 2.0 * M_PI;
                    let am_freq =
                        ((channel - center_freq_actual) as f32) * 2.0 * std::f32::consts::PI
                            / (rtl_in_rate as f32);
                    let mut window: Vec<num::Complex<f32>> = vec![];
                    for i in 0..self.rtl_mult {
                        // ch->wf[ind]=cexpf(AMFreq*ind*-I)/rtlMult/127.5;
                        let window_value = (am_freq * i as f32 * -num::complex::Complex::i()).exp()
                            / self.rtl_mult as f32
                            / 127.5;
                        window.push(window_value);
                    }
                    channel_windows.push(window);
                }

                for i in 0..self.frequencies.len() {
                    // create an array out of the channel_window[i] vector
                    let mut window_array: [num::Complex<f32>; 192] =
                        [num::complex::Complex::new(0.0, 0.0); 192];
                    for (ind, window_value) in channel_windows[i].iter().enumerate() {
                        window_array[ind] = *window_value;
                    }
                    let mut out_channel = ACARSDecoder::new(i as i32, channels[i], window_array);
                    out_channel.set_output_channel(output_channel.clone());

                    self.channel.push(Box::new(out_channel));
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

    pub async fn read_samples(mut self) {
        let rtloutbufz = self.get_rtloutbufsz();
        let buffer_len: u32 = rtloutbufz as u32 * self.rtl_mult as u32 * 2;
        let mut vb: [num::Complex<f32>; 320] = [num::complex::Complex::new(0.0, 0.0); 320];
        let mut counter: usize = 0;

        match self.reader {
            None => {
                error!("[{: <13}] Device not open", self.serial);
            }

            Some(mut reader) => {
                reader
                    .read_async(4, buffer_len, |bytes| {
                        counter = 0;
                        for m in 0..rtloutbufz {
                            for vb_item in vb.iter_mut().take(self.rtl_mult as usize) {
                                *vb_item = (bytes[counter] as f32 - 127.37)
                                    + (bytes[counter + 1] as f32 - 127.37)
                                        * num::complex::Complex::i();
                                counter += 2;
                            }

                            for channel in &mut self.channel {
                                let mut d: num::Complex<f32> = num::complex::Complex::new(0.0, 0.0);

                                for (ind, vb_item) in
                                    vb.iter().enumerate().take(self.rtl_mult as usize)
                                {
                                    d += vb_item * channel.get_wf_at_index(ind);
                                }

                                channel.set_dm_buffer_at_index(m, d.norm());
                            }
                        }

                        for channel in &mut self.channel {
                            channel.decode(rtloutbufz as u32);
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
