use rtlsdr_mt::{Controller, Reader};
use rtlsdr_sys;
extern crate libc;
use std::ffi::c_char;
use std::ffi::CStr;
use std::fmt::{self, Display, Formatter};

pub struct RtlSdr {
    ctl: Option<Controller>,
    reader: Option<Reader>,
    index: Option<u32>,
    serial: String,
}

impl RtlSdr {
    pub fn new(serial: String) -> Self {
        Self {
            ctl: None,
            reader: None,
            index: None,
            serial,
        }
    }

    pub fn open_sdr(&mut self) {
        let mut device_index = None;
        for dev in devices() {
            println!("{}", dev);
            if dev.serial() == &self.serial {
                device_index = Some(dev.index());
            }
        }
        match device_index {
            None => {
                println!("Device not found");
                return;
            }
            Some(idx) => {
                self.index = Some(idx);
                println!("Using Found device at index {}", idx);

                let (mut ctl, reader) = rtlsdr_mt::open(self.index.unwrap()).unwrap();

                self.reader = Some(reader);

                ctl.enable_agc().unwrap();
                ctl.set_ppm(-2).unwrap();
                ctl.set_center_freq(774_781_250).unwrap();

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

    println!("Found {} RTL-SDR devices", devices.len());

    devices.into_iter()
}
