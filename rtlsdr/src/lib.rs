#[macro_use]
extern crate log;
use rtlsdr_mt::{Controller, Reader};
use rtlsdr_sys;
extern crate libc;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};

const INTRATE: i32 = 12500;

pub struct RtlSdr {
    ctl: Option<Controller>,
    reader: Option<Reader>,
    index: Option<u32>,
    serial: String,
    ppm: i32,
    gain: i32,
    bias_tee: bool,
    rtl_mult: i32,
    frequencies: Vec<i32>,
}

impl RtlSdr {
    pub fn new(
        serial: String,
        ppm: i32,
        gain: i32,
        bias_tee: bool,
        rtl_mult: i32,
        frequencies: Vec<i32>,
    ) -> RtlSdr {
        frequencies.clone().sort();
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
        }
    }

    pub fn open_sdr(&mut self) {
        let mut device_index = None;
        for dev in devices() {
            if dev.serial() == &self.serial {
                device_index = Some(dev.index());
            }
        }
        match device_index {
            None => {
                error!("{} Device not found", self.serial);
                return;
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
                    for i in 0..32 {
                        if gains[i] == 0 {
                            continue;
                        }

                        let err1 = i32::abs(self.gain - close_gain);
                        let err2 = i32::abs(self.gain - gains[i]);

                        if err2 < err1 {
                            trace!("{} Found closer gain: {}", self.serial, gains[i]);
                            close_gain = gains[i];
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

                let rtl_in_rate = INTRATE * self.rtl_mult;
                let mut channels: Vec<i32> = Vec::new();

                for freq in self.frequencies.iter() {
                    channels.push(freq + INTRATE / 2);
                }
                channels.sort();

                // TODO: Make sure we're setting the center freq right....

                if self.frequencies.len() > 1 {
                    let center_freq =
                        (self.frequencies[self.frequencies.len() - 1] + self.frequencies[0]) / 2;
                    info!(
                        "{} Setting center frequency to {}",
                        self.serial, center_freq
                    );
                    ctl.set_center_freq(center_freq as u32).unwrap();
                } else {
                    info!(
                        "{} Setting center frequency to {}",
                        self.serial, self.frequencies[0]
                    );
                    ctl.set_center_freq(self.frequencies[0] as u32).unwrap();
                }

                info!("{} Setting sample rate to {}", self.serial, rtl_in_rate);
                ctl.set_sample_rate(rtl_in_rate as u32).unwrap();

                self.ctl = Some(ctl);

                // std::thread::spawn(move || loop {
                //     let next = self.ctl.center_freq() + 1000;
                //     self.ctl.set_center_freq(next).unwrap();

                //     std::thread::sleep(std::time::Duration::from_secs(1));
                // });

                // self.reader
                //     .read_async(4, 32768, |bytes| {
                //         println!("i[0] = {}", bytes[0]);
                //         println!("q[0] = {}", bytes[1]);
                //     })
                //     .unwrap();
            }
        }
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
    let count = unsafe { rtlsdr_sys::rtlsdr_get_device_count() } as u32;

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

    devices.into_iter()
}
