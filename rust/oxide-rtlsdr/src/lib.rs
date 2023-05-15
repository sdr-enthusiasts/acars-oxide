#[macro_use]
extern crate log;
use rtlsdr_mt::{Controller, Reader};

extern crate libc;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};
use std::ops::Add;

const INTRATE: i32 = 12500;
const RTLOUTBUFSZ: usize = 1024;
const FLEN: i32 = (INTRATE / 1200) + 1;
const MFLTOVER: usize = 12;
const FLENO: usize = FLEN as usize * MFLTOVER + 1;
const PLLG: f32 = 38e-4;
const PLLC: f32 = 0.52;

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
    MskPhi: f32,
    MskDf: f32,
    MskClk: f32,
    MskLvlSum: f32,
    MskBitCount: i32,
    MskS: u32,
    idx: u32,
    inb: [num::Complex<f32>; FLEN as usize],

    outbits: u32, // orignial was unsigned char.....
    nbits: i32,
    acars_state: ACARSState,
    h: [f32; FLENO],
}

impl Channel {
    pub fn new(channel_number: i32, freq: i32, wf: Vec<num::Complex<f32>>) -> Self {
        let mut h: [f32; FLENO] = [0.0; FLENO];
        for i in 0..FLENO - 1 {
            h[i] = f32::cos(
                2.0 * std::f32::consts::PI * 600.0 / INTRATE as f32 / MFLTOVER as f32
                    * (i as f32 - (FLENO as f32 - 1.0) / 2.0),
            );
            if h[i] < 0.0 {
                h[i] = 0.0;
            };
        }

        Self {
            channel_number,
            freq,
            wf,
            dm_buffer: vec![0.0; RTLOUTBUFSZ],
            MskPhi: 0.0,
            MskDf: 0.0,
            MskClk: 0.0,
            MskLvlSum: 0.0,
            MskBitCount: 0,
            MskS: 0,
            idx: 0,
            inb: [num::Complex::new(0.0, 0.0); FLEN as usize],
            outbits: 0,
            nbits: 8,
            acars_state: ACARSState::WSYN,
            h: h,
        }
    }

    pub fn demodMSK(&mut self, len: u32) {
        /* MSK demod */

        for n in 0..len - 1 {
            let in_: f32;
            let s: f32;
            let mut v: num::Complex<f32> = num::Complex::new(0.0, 0.0);
            let mut o: f32;

            /* VCO */
            s = 1800.0 / INTRATE as f32 * 2.0 * std::f32::consts::PI + self.MskDf as f32;
            self.MskPhi += s;
            if self.MskPhi >= 2.0 * std::f32::consts::PI {
                self.MskPhi -= 2.0 * std::f32::consts::PI
            };

            /* mixer */
            in_ = self.dm_buffer[n as usize];
            self.inb[self.idx as usize] = in_ * num::Complex::exp(-self.MskPhi * num::Complex::i());
            self.idx = (self.idx + 1) % (FLEN as u32);

            /* bit clock */
            self.MskClk += s;
            if self.MskClk >= 3.0 * std::f32::consts::PI / 2.0 - s / 2.0 {
                let dphi: f32;
                let vo: f32;
                let lvl: f32;

                self.MskClk -= 3.0 * std::f32::consts::PI / 2.0;

                /* matched filter */
                o = MFLTOVER as f32 * (self.MskClk / s + 0.5);
                if o > MFLTOVER as f32 {
                    o = MFLTOVER as f32
                };
                // for (v = 0, j = 0; j < FLEN; j++,o+=MFLTOVER) {
                // 	v += h[o]*ch->inb[(j+idx)%FLEN];
                // }

                let mut j = 0;

                while j < FLEN {
                    v = v.add(
                        self.h[o as usize]
                            * self.inb[(j as usize + self.idx as usize) % FLEN as usize],
                    );
                    j += 1;
                    o += MFLTOVER as f32;
                }

                /* normalize */
                lvl = v.norm();
                v /= lvl + 1e-8;
                self.MskLvlSum += lvl * lvl / 4.0;
                self.MskBitCount += 1;

                if self.MskS & 1 != 0 {
                    vo = v.im;
                    if vo >= 0.0 {
                        dphi = -v.re;
                    } else {
                        dphi = v.re;
                    };
                } else {
                    vo = v.re;
                    if vo >= 0.0 {
                        dphi = v.im;
                    } else {
                        dphi = -v.im;
                    };
                }
                if self.MskS & 2 != 0 {
                    self.put_bit(-vo);
                } else {
                    self.put_bit(vo);
                }
                self.MskS += 1;

                /* PLL filter */
                self.MskDf = PLLC * self.MskDf + (1.0 - PLLC) * PLLG * dphi;
            }
        }
    }

    fn put_bit(&mut self, bit: f32) {
        self.outbits >>= 1;
        if bit > 0.0 {
            self.outbits |= 0x80;
        }

        self.nbits -= 1;
        if self.nbits <= 0 {
            // DECODE ACARS!
            self.nbits = 8;
        }
    }
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

                        for channel_index in 0..self.channel.len() - 1 {
                            self.channel[channel_index].demodMSK(RTLOUTBUFSZ as u32);
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
