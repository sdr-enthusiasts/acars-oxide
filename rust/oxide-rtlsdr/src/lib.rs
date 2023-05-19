#[macro_use]
extern crate log;
use rtlsdr_mt::{Controller, Reader};

extern crate libc;

use custom_error::custom_error;
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
const MAXPERR: usize = 3;

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

const CRC: [u8; 256] = [
    0x0000, 0x1189, 0x2312, 0x329b, 0x4624, 0x57ad, 0x6536, 0x74bf, 0x8c48, 0x9dc1, 0xaf5a, 0xbed3,
    0xca6c, 0xdbe5, 0xe97e, 0xf8f7, 0x1081, 0x0108, 0x3393, 0x221a, 0x56a5, 0x472c, 0x75b7, 0x643e,
    0x9cc9, 0x8d40, 0xbfdb, 0xae52, 0xdaed, 0xcb64, 0xf9ff, 0xe876, 0x2102, 0x308b, 0x0210, 0x1399,
    0x6726, 0x76af, 0x4434, 0x55bd, 0xad4a, 0xbcc3, 0x8e58, 0x9fd1, 0xeb6e, 0xfae7, 0xc87c, 0xd9f5,
    0x3183, 0x200a, 0x1291, 0x0318, 0x77a7, 0x662e, 0x54b5, 0x453c, 0xbdcb, 0xac42, 0x9ed9, 0x8f50,
    0xfbef, 0xea66, 0xd8fd, 0xc974, 0x4204, 0x538d, 0x6116, 0x709f, 0x0420, 0x15a9, 0x2732, 0x36bb,
    0xce4c, 0xdfc5, 0xed5e, 0xfcd7, 0x8868, 0x99e1, 0xab7a, 0xbaf3, 0x5285, 0x430c, 0x7197, 0x601e,
    0x14a1, 0x0528, 0x37b3, 0x263a, 0xdecd, 0xcf44, 0xfddf, 0xec56, 0x98e9, 0x8960, 0xbbfb, 0xaa72,
    0x6306, 0x728f, 0x4014, 0x519d, 0x2522, 0x34ab, 0x0630, 0x17b9, 0xef4e, 0xfec7, 0xcc5c, 0xddd5,
    0xa96a, 0xb8e3, 0x8a78, 0x9bf1, 0x7387, 0x620e, 0x5095, 0x411c, 0x35a3, 0x242a, 0x16b1, 0x0738,
    0xffcf, 0xee46, 0xdcdd, 0xcd54, 0xb9eb, 0xa862, 0x9af9, 0x8b70, 0x8408, 0x9581, 0xa71a, 0xb693,
    0xc22c, 0xd3a5, 0xe13e, 0xf0b7, 0x0840, 0x19c9, 0x2b52, 0x3adb, 0x4e64, 0x5fed, 0x6d76, 0x7cff,
    0x9489, 0x8500, 0xb79b, 0xa612, 0xd2ad, 0xc324, 0xf1bf, 0xe036, 0x18c1, 0x0948, 0x3bd3, 0x2a5a,
    0x5ee5, 0x4f6c, 0x7df7, 0x6c7e, 0xa50a, 0xb483, 0x8618, 0x9791, 0xe32e, 0xf2a7, 0xc03c, 0xd1b5,
    0x2942, 0x38cb, 0x0a50, 0x1bd9, 0x6f66, 0x7eef, 0x4c74, 0x5dfd, 0xb58b, 0xa402, 0x9699, 0x8710,
    0xf3af, 0xe226, 0xd0bd, 0xc134, 0x39c3, 0x284a, 0x1ad1, 0x0b58, 0x7fe7, 0x6e6e, 0x5cf5, 0x4d7c,
    0xc60c, 0xd785, 0xe51e, 0xf497, 0x8028, 0x91a1, 0xa33a, 0xb2b3, 0x4a44, 0x5bcd, 0x6956, 0x78df,
    0x0c60, 0x1de9, 0x2f72, 0x3efb, 0xd68d, 0xc704, 0xf59f, 0xe416, 0x90a9, 0x8120, 0xb3bb, 0xa232,
    0x5ac5, 0x4b4c, 0x79d7, 0x685e, 0x1ce1, 0x0d68, 0x3ff3, 0x2e7a, 0xe70e, 0xf687, 0xc41c, 0xd595,
    0xa12a, 0xb0a3, 0x8238, 0x93b1, 0x6b46, 0x7acf, 0x4854, 0x59dd, 0x2d62, 0x3ceb, 0x0e70, 0x1ff9,
    0xf78f, 0xe606, 0xd49d, 0xc514, 0xb1ab, 0xa022, 0x92b9, 0x8330, 0x7bc7, 0x6a4e, 0x58d5, 0x495c,
    0x3de3, 0x2c6a, 0x1ef1, 0x0f78,
];

custom_error! {pub RTLSDRError
    DeviceNotFound { sdr: String } = "Device {sdr} not found",
    FrequencySpreadTooLarge { sdr: String } = "Frequency spread too large for device {sdr}. Must be less than 2mhz",
    NoFrequencyProvided { sdr: String } = "No frequency provided for device {sdr}",
}

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

struct Mskblks {
    chn: i32,
    timeval: u64,
    len: i32,
    pub err: usize,
    lvl: f32,
    pub txt: [u8; 250],
    pub crc: [u8; 2],
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
            txt: [0; 250],
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
        self.txt = [0; 250];
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

    pub fn set_err(&mut self, err: usize) {
        self.err = err;
    }

    pub fn set_lvl(&mut self, lvl: f32) {
        self.lvl = lvl;
    }

    pub fn set_txt(&mut self, txt: [u8; 250]) {
        self.txt = txt;
    }

    pub fn set_text_by_index(&mut self, index: usize, txt: u8) {
        self.txt[index] = txt;
    }

    pub fn set_crc(&mut self, crc: [u8; 2]) {
        self.crc = crc;
    }

    pub fn len(&self) -> i32 {
        self.len
    }
}

struct Channel {
    channel_number: i32,
    freq: i32,
    wf: [num::Complex<f32>; 192],
    dm_buffer: [f32; RTLOUTBUFSZ],
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
    pub fn new(channel_number: i32, freq: i32, wf: [num::Complex<f32>; 192]) -> Self {
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
            dm_buffer: [0.0; RTLOUTBUFSZ],
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
            h,
            blk: Mskblks::new(),
        }
    }

    pub fn demod_msk(&mut self, len: u32) {
        /* MSK demod */

        for n in 0..len as usize {
            let in_: f32 = self.dm_buffer[n];
            let s: f32 = 1800.0 / INTRATE as f32 * 2.0 * std::f32::consts::PI + self.msk_df;
            let mut v: num::Complex<f32> = num::Complex::new(0.0, 0.0);
            let mut o: f32;

            /* VCO */
            self.msk_phi += s;
            if self.msk_phi >= 2.0 * std::f32::consts::PI {
                self.msk_phi -= 2.0 * std::f32::consts::PI
            };

            /* mixer */

            self.inb[self.idx as usize] =
                in_ * num::Complex::exp(-self.msk_phi * num::Complex::i());
            self.idx = (self.idx + 1) % (FLEN as u32);

            /* bit clock */
            self.msk_clk += s;
            if self.msk_clk >= 3.0 * std::f32::consts::PI / 2.0 - s / 2.0 {
                let dphi: f32;
                let vo: f32;

                self.msk_clk -= 3.0 * std::f32::consts::PI / 2.0;

                /* matched filter */
                o = MFLTOVER as f32 * (self.msk_clk / s + 0.5);
                if o > MFLTOVER as f32 {
                    o = MFLTOVER as f32
                };

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
                let lvl: f32 = v.norm();
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
            }
            ACARSState::TXT => {
                info!("TXT!");
                self.blk
                    .set_text_by_index(self.blk.len as usize, self.outbits);
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
            }
            ACARSState::CRC1 => {
                info!("CRC1");
                self.blk.crc[0] = self.outbits;
                self.acars_state = ACARSState::CRC2;
                self.nbits = 8;
            }

            ACARSState::CRC2 => {
                info!("CRC2");
                self.blk.crc[1] = self.outbits;
                self.put_msg_label();
            }
            ACARSState::END => {
                info!("END");
                self.reset_acars();
                self.nbits = 8;
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

    fn process_acars_message(&mut self, blk: &mut Mskblks) {
        let mut pr: [u8; 3] = [0; 3];
        // handle message
        if blk.len() < 13 {
            // too short
            info!("Message too short");
            return;
        }

        /* force STX/ETX */
        blk.txt[12] &= ETX | STX;
        blk.txt[12] |= ETX & STX;

        /* parity check */
        let mut pn: usize = 0;
        for i in 0..blk.len as usize {
            if (NUMBITS[blk.txt[i] as usize] & 1) == 0 {
                if pn < MAXPERR {
                    pr[pn] = i as u8;
                }
                pn += 1;
            }
            if (NUMBITS[blk.txt[i] as usize] & 1) == 0 {
                if pn < MAXPERR {
                    pr[pn] = i as u8;
                }
                pn += 1;
            }
        }
        if pn > MAXPERR {
            info!("Too many parity errors");
            return;
        }
        if pn > 0 {
            info!("Parity errors: {}", pn);
        }
        blk.err = pn;

        /* crc check */
        let mut crc: u8 = 0;
        for i in 0..blk.len as usize {
            crc = update_crc(crc, blk.txt[i]);
        }

        update_crc(crc, blk.crc[0]);
        update_crc(crc, blk.crc[1]);
        if crc != 0 {
            error!("{} crc error\n", blk.chn + 1);
        } else {
            info!("CRC OK");
        }

        /* try to fix error */
        // if(pn) {
        //   if (fixprerr(blk, crc, pr, pn) == 0) {
        // 	if (verbose)
        // 		fprintf(stderr, "#%d not able to fix errors\n", blk->chn + 1);
        // 	free(blk);
        // 	continue;
        //   }
        // 	if (verbose)
        // 		fprintf(stderr, "#%d errors fixed\n", blk->chn + 1);
        // } else {

        //   if (crc) {
        // 	 if(fixdberr(blk, crc) == 0) {
        // 		if (verbose)
        // 			fprintf(stderr, "#%d not able to fix errors\n", blk->chn + 1);
        // 		free(blk);
        // 		continue;
        //   	}
        //   	if (verbose)
        // 		fprintf(stderr, "#%d errors fixed\n", blk->chn + 1);
        //   }
        // }

        // /* redo parity checking and removing */
        // pn = 0;
        // for (i = 0; i < blk->len; i++) {
        // 	if ((numbits[(unsigned char)(blk->txt[i])] & 1) == 0) {
        // 		pn++;
        // 	}
        // 	blk->txt[i] &= 0x7f;
        // }
        // if (pn) {
        // 	fprintf(stderr, "#%d parity check problem\n",
        // 		blk->chn + 1);
        // 	free(blk);
        // 	continue;
        // }

        // outputmsg(blk);
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

    pub fn open_sdr(&mut self) -> Result<(), RTLSDRError> {
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
                info!("{} Using Found device at index {}", self.serial, idx);

                let (mut ctl, reader) = rtlsdr_mt::open(self.index.unwrap()).unwrap();

                self.reader = Some(reader);

                // remove any duplicate frequencies
                // I cannot imagine we would EVER see this, but just in case

                self.frequencies.dedup();
                if self.frequencies.len() == 0 {
                    return Err(RTLSDRError::NoFrequencyProvided {
                        sdr: self.serial.clone(),
                    });
                }

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
                // Verify freq spread less than 2mhz. This is much less complex than acarsdec
                // but I fail to see how this is not equivilant with a lot less bullshit

                if self.frequencies.len() > 1 {
                    if self.frequencies[self.frequencies.len() - 1] - self.frequencies[0] > 2.0 {
                        return Err(RTLSDRError::FrequencySpreadTooLarge {
                            sdr: self.serial.clone(),
                        });
                    }
                }

                let rtl_in_rate = INTRATE * self.rtl_mult;
                let mut channels: Vec<i32> = Vec::new();

                for freq in self.frequencies.iter() {
                    let channel = ((((1000000.0 * freq) as i32) + INTRATE / 2) / INTRATE) * INTRATE;
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
                    // create an array out of the channel_window[i] vector
                    let mut window_array: [num::Complex<f32>; 192] =
                        [num::complex::Complex::new(0.0, 0.0); 192];
                    for (ind, window_value) in channel_windows[i].iter().enumerate() {
                        window_array[ind] = *window_value;
                    }
                    let out_channel = Channel::new(i as i32, channels[i], window_array);

                    self.channel.push(out_channel);
                }

                info!("{} Setting sample rate to {}", self.serial, rtl_in_rate);
                ctl.set_sample_rate(rtl_in_rate as u32).unwrap();

                self.ctl = Some(ctl);
            }
        };

        Ok(())
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
        let mut vb: [num::Complex<f32>; 320] = [num::complex::Complex::new(0.0, 0.0); 320];
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

                                for (ind, vb_item) in
                                    vb.iter().enumerate().take(self.rtl_mult as usize)
                                {
                                    d += vb_item * channel.wf[ind];
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

fn update_crc(crc: u8, c: u8) -> u8 {
    // #define update_crc(crc,c) crc= (crc>> 8)^crc_ccitt_table[(crc^(c))&0xff];
    let mut crc: u8 = crc;
    crc = (crc >> 8) ^ CRC[((crc ^ c) & 0xff) as usize];
    crc as u8
}
