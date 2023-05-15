#[macro_use]
extern crate log;
use rtlsdr_mt::{Controller, Reader};

extern crate libc;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};

const INTRATE: i32 = 12500;
const RTLOUTBUFSZ: usize = 1024;

enum ACARSState {
    WSYN,
    SYN2,
    SOH1,
    TXT,
    CRC1,
    CRC2,
    END,
}

struct Channel {
    channel_number: i32,
    freq: i32,
    wf: Vec<num::Complex<f32>>,
    dm_buffer: Vec<f32>,
    MskPhi: i32,
    MskDf: i32,
    MskClk: f32,
    MskLvlSum: f32,
    MskBitCount: i32,
    MskS: u32,
    idx: u32,
    inb: num::Complex<f32>,

    outbits: Vec<u8>, // orignial was unsigned char.....
    nbits: i32,
    acars_state: ACARSState,
}

impl Channel {
    pub fn new(channel_number: i32, freq: i32, wf: Vec<num::Complex<f32>>) -> Self {
        Self {
            channel_number,
            freq,
            wf,
            dm_buffer: vec![0.0; RTLOUTBUFSZ],
            MskPhi: 0,
            MskDf: 0,
            MskClk: 0.0,
            MskLvlSum: 0.0,
            MskBitCount: 0,
            MskS: 0,
            idx: 0,
            inb: num::Complex::new(0.0, 0.0),
            outbits: Vec::new(),
            nbits: 0,
            acars_state: ACARSState::WSYN,
        }
    }

    pub fn demodMSK(len: u32) {}
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
    channel: Vec<Channel>,
}

impl RtlSdr {
    pub fn new(
        serial: String,
        ppm: i32,
        gain: i32,
        bias_tee: bool,
        rtl_mult: i32,
        mut frequencies: Vec<f32>,
    ) -> RtlSdr {
        frequencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        debug!("{} Frequencies: {:?}", serial, frequencies);
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
        }
    }

    pub fn open_sdr(&mut self) {
        let mut device_index = None;
        for dev in devices() {
            if dev.serial() == self.serial {
                device_index = Some(dev.index());
            }
        }
        match device_index {
            None => {
                error!("{} Device not found", self.serial);
            }
            Some(idx) => {
                self.index = Some(idx);
                info!("{} Using Found device at index {}", self.serial, idx);

                let (mut ctl, reader) = rtlsdr_mt::open(self.index.unwrap()).unwrap();

                self.reader = Some(reader);

                if self.gain <= 500 {
                    let mut gains = [0i32; 32];
                    ctl.tuner_gains(&mut gains);
                    debug!("{} Using Gains: {:?}", self.serial, gains);
                    let mut close_gain = gains[0];
                    // loop through gains and see which value is closest to the desired gain
                    for gain_value in gains {
                        if gain_value == 0 {
                            continue;
                        }

                        let err1 = i32::abs(self.gain - close_gain);
                        let err2 = i32::abs(self.gain - gain_value);

                        if err2 < err1 {
                            trace!("{} Found closer gain: {}", self.serial, gain_value);
                            close_gain = gain_value;
                        }
                    }

                    if self.gain != close_gain {
                        warn!(
                            "{} Input gain {} was normalized to a SDR supported gain of {}. Gain is set to the normalized gain.",
                            self.serial, self.gain, close_gain
                        );
                        self.gain = close_gain;
                    } else {
                        info!("{} setting gain to {}", self.serial, self.gain);
                    }

                    ctl.disable_agc().unwrap();
                    ctl.set_tuner_gain(self.gain).unwrap();
                } else {
                    info!("{} Setting gain to Auto Gain Control (AGC)", self.serial);
                    ctl.enable_agc().unwrap();
                }

                info!("{} Setting PPM to {}", self.serial, self.ppm);
                ctl.set_ppm(self.ppm).unwrap();

                if self.bias_tee {
                    warn!(
                        "{} BiasTee is not supported right now. Maybe soon...",
                        self.serial
                    );
                }
                // TODO: verify < 2mhz spread in bandwidth
                let rtl_in_rate = INTRATE * self.rtl_mult;
                let mut channels: Vec<i32> = Vec::new();

                for freq in self.frequencies.iter() {
                    // ((int)(1000000 * atof(argF) + INTRATE / 2) / INTRATE) * INTRATE;
                    let channel = ((((1000000.0 * freq) as i32) + INTRATE / 2) / INTRATE) * INTRATE;
                    channels.push(channel);
                }

                // TODO: Make sure we're setting the center freq right....

                let center_freq_actual;

                if channels.len() > 1 {
                    let center_freq = (channels[channels.len() - 1] + channels[0]) / 2;
                    info!(
                        "{} Setting center frequency to {}",
                        self.serial, center_freq
                    );
                    center_freq_actual = center_freq;
                    ctl.set_center_freq(center_freq as u32).unwrap();
                } else {
                    info!(
                        "{} Setting center frequency to {}",
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
                        // TODO: Did I fuck this up? Complex::i()?
                        let window_value = (am_freq * i as f32 * -num::complex::Complex::i())
                            / self.rtl_mult as f32
                            / 127.5;
                        window.push(window_value);
                    }
                    channel_windows.push(window);
                }

                for i in 0..self.frequencies.len() - 1 {
                    let out_channel =
                        Channel::new(i as i32, channels[i], channel_windows[i].clone());

                    self.channel.push(out_channel);
                }

                info!("{} Setting sample rate to {}", self.serial, rtl_in_rate);
                ctl.set_sample_rate(rtl_in_rate as u32).unwrap();

                self.ctl = Some(ctl);
            }
        }
    }

    pub fn close_sdr(self) {
        match self.ctl {
            None => {
                error!("{} Device not open", self.serial);
            }
            Some(mut ctl) => {
                ctl.cancel_async_read();
            }
        }
    }

    pub async fn read_samples(mut self) {
        let buffer_len = RTLOUTBUFSZ as u32 * self.rtl_mult as u32 * 2;
        let mut vb: Vec<num::Complex<f32>> = vec![num::complex::Complex::new(0.0, 0.0); 320];
        match self.reader {
            None => {
                error!("{} Device not open", self.serial);
            }
            Some(mut reader) => {
                reader
                    .read_async(4, buffer_len, |bytes| {
                        let mut counter = 0;
                        for m in 0..RTLOUTBUFSZ {
                            for u in 0..self.rtl_mult - 1 {
                                let r: f32;
                                let g: f32;

                                r = bytes[counter] as f32 - 127.37;
                                counter += 1;
                                g = bytes[counter] as f32 - 127.37;
                                counter += 1;

                                vb[u as usize] = r + g * num::complex::Complex::i();
                            }

                            for channel_index in 0..self.channel.len() - 1 {
                                let mut d: num::Complex<f32> = num::complex::Complex::new(0.0, 0.0);
                                for ind in 0..self.rtl_mult - 1 {
                                    d += vb[ind as usize]
                                        * self.channel[channel_index].wf[ind as usize];
                                }

                                self.channel[channel_index].dm_buffer[m] = d.norm();
                            }
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

    info!("Found {} RTL-SDR devices", devices.len());
    for device in devices.iter() {
        debug!("{}", device);
    }

    devices.into_iter()
}
