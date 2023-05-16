#[macro_use]
extern crate log;
use rtlsdr_mt::{Controller, Reader};

extern crate libc;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};
use std::ops::Add;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

const INTRATE: i32 = 12500;
const RTLOUTBUFSZ: usize = 1024;
const FLEN: i32 = (INTRATE / 1200) + 1;
const MFLTOVER: usize = 12;
const FLENO: usize = FLEN as usize * MFLTOVER + 1;
const PLLG: f32 = 38e-4;
const PLLC: f32 = 0.52;
const SYN: u8 = 0x16;
const SOH: u8 = 0x01;
const STX: u8 = 0x02;
const ETX: u8 = 0x83;
const ETB: u8 = 0x97;
const DLE: u8 = 0x7f;
const MAXPERR: i32 = 3;

const NUMBITS: [u8; 256] = [
    0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4, 1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5,
    1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5, 2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
    1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5, 2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
    2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
    1, 2, 2, 3, 2, 3, 3, 4, 2, 3, 3, 4, 3, 4, 4, 5, 2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6,
    2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
    2, 3, 3, 4, 3, 4, 4, 5, 3, 4, 4, 5, 4, 5, 5, 6, 3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7,
    3, 4, 4, 5, 4, 5, 5, 6, 4, 5, 5, 6, 5, 6, 6, 7, 4, 5, 5, 6, 5, 6, 6, 7, 5, 6, 6, 7, 6, 7, 7, 8,
];

#[derive(Debug)]
enum ACARSState {
    WSYN,
    SYN2,
    SOH1,
    TXT,
    CRC1,
    CRC2,
    END,
}

// typedef struct mskblk_s {
// 	struct mskblk_s *prev;
// 	int chn;
// 	struct timeval tv;
// 	int len;
// 	int err;
// 	float lvl;
// 	char txt[250];
// 	unsigned char crc[2];
// } msgblk_t;

struct Mskblks {
    chn: i32,
    timeval: u64,
    len: i32,
    err: i32,
    lvl: f32,
    txt: [char; 250],
    crc: [u8; 2],
    prev: Option<Box<Mskblks>>,
}

impl Mskblks {
    pub fn new() -> Self {
        Self {
            chn: 0,
            timeval: 0,
            len: 0,
            err: 0,
            lvl: 0.0,
            txt: ['\0'; 250],
            crc: [0; 2],
            prev: None,
        }
    }

    pub fn reset(&mut self) {
        self.chn = 0;
        self.timeval = 0;
        self.len = 0;
        self.err = 0;
        self.lvl = 0.0;
        self.txt = ['\0'; 250];
        self.crc = [0; 2];
        self.prev = None;
    }

    pub fn set_chn(&mut self, chn: i32) {
        self.chn = chn;
    }

    pub fn set_timeval(&mut self, timeval: u64) {
        self.timeval = timeval;
    }

    pub fn set_len(&mut self, len: i32) {
        self.len = len;
    }

    pub fn set_err(&mut self, err: i32) {
        self.err = err;
    }

    pub fn set_lvl(&mut self, lvl: f32) {
        self.lvl = lvl;
    }

    pub fn set_txt(&mut self, txt: [char; 250]) {
        self.txt = txt;
    }

    pub fn set_text_by_index(&mut self, index: usize, txt: char) {
        self.txt[index] = txt;
    }

    pub fn set_crc(&mut self, crc: [u8; 2]) {
        self.crc = crc;
    }
}

struct Channel {
    channel_number: i32,
    freq: i32,
    wf: Vec<num::Complex<f32>>,
    dm_buffer: Vec<f32>,
    msk_phi: f32,
    msk_df: f32,
    msk_clk: f32,
    msk_lvl_sum: f32,
    msk_bit_count: i32,
    msk_s: u32,
    idx: u32,
    inb: [num::Complex<f32>; FLEN as usize],
    outbits: u8, // orignial was unsigned char.....
    nbits: i32,
    acars_state: ACARSState,
    h: [f32; FLENO],
    blk: Mskblks,
}

impl Channel {
    pub fn new(channel_number: i32, freq: i32, wf: Vec<num::Complex<f32>>) -> Self {
        let mut h: [f32; FLENO] = [0.0; FLENO];
        for i in 0..FLENO {
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
            msk_phi: 0.0,
            msk_df: 0.0,
            msk_clk: 0.0,
            msk_lvl_sum: 0.0,
            msk_bit_count: 0,
            msk_s: 0,
            idx: 0,
            inb: [num::Complex::new(0.0, 0.0); FLEN as usize],
            outbits: 0,
            nbits: 8,
            acars_state: ACARSState::WSYN,
            h: h,
            blk: Mskblks::new(),
        }
    }

    pub fn demod_msk(&mut self, len: u32) {
        /* MSK demod */

        for n in 0..len {
            let in_: f32;
            let s: f32;
            let mut v: num::Complex<f32> = num::Complex::new(0.0, 0.0);
            let mut o: f32;

            /* VCO */
            s = 1800.0 / INTRATE as f32 * 2.0 * std::f32::consts::PI + self.msk_df as f32;
            self.msk_phi += s;
            if self.msk_phi >= 2.0 * std::f32::consts::PI {
                self.msk_phi -= 2.0 * std::f32::consts::PI
            };

            /* mixer */
            in_ = self.dm_buffer[n as usize];
            self.inb[self.idx as usize] =
                in_ * num::Complex::exp(-self.msk_phi * num::Complex::i());
            self.idx = (self.idx + 1) % (FLEN as u32);

            /* bit clock */
            self.msk_clk += s;
            if self.msk_clk >= 3.0 * std::f32::consts::PI / 2.0 - s / 2.0 {
                let dphi: f32;
                let vo: f32;
                let lvl: f32;

                self.msk_clk -= 3.0 * std::f32::consts::PI / 2.0;

                /* matched filter */
                o = MFLTOVER as f32 * (self.msk_clk / s + 0.5);
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
                self.msk_lvl_sum += lvl * lvl / 4.0;
                self.msk_bit_count += 1;

                if self.msk_s & 1 != 0 {
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
                if self.msk_s & 2 != 0 {
                    self.put_bit(-vo);
                } else {
                    self.put_bit(vo);
                }
                self.msk_s += 1;

                /* PLL filter */
                self.msk_df = PLLC * self.msk_df + (1.0 - PLLC) * PLLG * dphi;
            }
        }
    }

    fn reset_acars(&mut self) {
        self.acars_state = ACARSState::WSYN;
        self.nbits = 8;
        self.nbits = 0;
    }

    fn put_bit(&mut self, bit: f32) {
        self.outbits >>= 1;
        if bit > 0.0 {
            self.outbits |= 0x80;
        }

        self.nbits -= 1;
        if self.nbits <= 0 {
            // DECODE ACARS!
            self.decode_acars();
            self.nbits = 8;
        }
    }

    fn decode_acars(&mut self) {
        //info!("{:?}", self.acars_state);
        match self.acars_state {
            ACARSState::WSYN => {
                if self.outbits == SYN {
                    self.acars_state = ACARSState::SYN2;
                    self.nbits = 8;
                    return;
                }
                // NOTE: This is supposed to be a bitwise NOT
                if self.outbits == !SYN {
                    self.msk_s ^= 2;
                    self.acars_state = ACARSState::SYN2;
                    self.nbits = 8;
                    return;
                }
                self.nbits = 1;
            }

            ACARSState::SYN2 => {
                if self.outbits == SYN {
                    self.acars_state = ACARSState::SOH1;
                    self.nbits = 8;
                    return;
                }
                // NOTE: This is supposed to be a bitwise NOT
                if self.outbits == !SYN {
                    self.msk_s ^= 2;
                    self.nbits = 8;
                    return;
                }
                self.reset_acars();
                return;
            }
            ACARSState::SOH1 => {
                info!(
                    "SOH1 {:x} {:x}, Same {:?}",
                    self.outbits,
                    SOH,
                    self.outbits == SOH,
                );
                if self.outbits == SOH {
                    self.blk.reset();

                    self.blk.set_chn(self.channel_number);
                    self.blk.set_timeval(
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs(),
                    );
                    self.blk.set_len(0);
                    self.blk.set_err(0);

                    self.acars_state = ACARSState::TXT;
                    self.nbits = 8;
                    self.msk_lvl_sum = 0.0;
                    self.msk_bit_count = 0;
                    return;
                }
                self.reset_acars();
                return;
            }
            ACARSState::TXT => {
                info!("TXT!");
                self.blk
                    .set_text_by_index(self.blk.len as usize, self.outbits as char);
                self.blk.len += 1;

                if (NUMBITS[self.outbits as usize] & 1) == 0 {
                    self.blk.err += 1;

                    if self.blk.err > MAXPERR + 1 {
                        self.reset_acars();
                        return;
                    }
                }
                if self.outbits == ETX || self.outbits == ETB {
                    self.acars_state = ACARSState::CRC1;
                    self.nbits = 8;
                    return;
                }
                if self.blk.len > 20 && self.outbits == DLE {
                    self.blk.len -= 3;
                    self.blk.crc[0] = self.blk.txt[self.blk.len as usize] as u8;
                    self.blk.crc[1] = self.blk.txt[self.blk.len as usize + 1] as u8;
                    self.acars_state = ACARSState::CRC2;
                    self.put_msg_label();
                }
                if self.blk.len > 240 {
                    self.reset_acars();
                    return;
                }
                self.nbits = 8;
                return;
            }
            ACARSState::CRC1 => {
                info!("CRC1");
                self.blk.crc[0] = self.outbits as u8;
                self.acars_state = ACARSState::CRC2;
                self.nbits = 8;
                return;
            }

            ACARSState::CRC2 => {
                info!("CRC2");
                self.blk.crc[1] = self.outbits as u8;
                self.put_msg_label();

                return;
            }
            ACARSState::END => {
                info!("END");
                self.reset_acars();
                self.nbits = 8;
                return;
            }
        }
    }

    fn put_msg_label(&mut self) {
        self.blk.lvl = 10.0 * (self.msk_lvl_sum / self.msk_bit_count as f32).log10();

        self.blk.prev = None;
        // THis is for message queueing, I think.
        // if (blkq_s)
        //     blkq_s->prev = Some(self.blk);
        // blkq_s = ch->blk;
        // if (blkq_e == NULL)
        //     blkq_e = blkq_s;

        info!("A message?");
        self.blk.reset();
        self.acars_state = ACARSState::END;
        self.nbits = 8;
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

                let center_freq_actual: i32;

                if channels.len() > 1 {
                    let center_freq_as_float = ((self.frequencies[self.frequencies.len() - 1]
                        + self.frequencies[0])
                        / 2.0)
                        .round();
                    let center_freq = (center_freq_as_float * 1000000.0) as i32;
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
                        let window_value = (am_freq * i as f32 * -num::complex::Complex::i()).exp()
                            / self.rtl_mult as f32
                            / 127.5;
                        window.push(window_value);
                    }
                    channel_windows.push(window);
                }

                for i in 0..self.frequencies.len() {
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
        let buffer_len: u32 = RTLOUTBUFSZ as u32 * self.rtl_mult as u32 * 2;
        let mut vb: Vec<num::Complex<f32>> = vec![num::complex::Complex::new(0.0, 0.0); 320];
        let mut counter: usize = 0;

        match self.reader {
            None => {
                error!("{} Device not open", self.serial);
            }

            Some(mut reader) => {
                reader
                    .read_async(4, buffer_len, |bytes| {
                        counter = 0;
                        for m in 0..RTLOUTBUFSZ {
                            for u in 0..self.rtl_mult as usize {
                                vb[u] = (bytes[counter] as f32 - 127.37)
                                    + (bytes[counter + 1] as f32 - 127.37)
                                        * num::complex::Complex::i();
                                counter += 2;
                            }

                            for channel in &mut self.channel {
                                let mut d: num::Complex<f32> = num::complex::Complex::new(0.0, 0.0);

                                for ind in 0..self.rtl_mult as usize {
                                    d += vb[ind] * channel.wf[ind];
                                }

                                channel.dm_buffer[m] = d.norm();
                            }
                        }

                        for channel_index in 0..self.channel.len() {
                            self.channel[channel_index].demod_msk(RTLOUTBUFSZ as u32);
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
