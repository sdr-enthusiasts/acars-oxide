#[macro_use]
extern crate log;
extern crate oxide_rtlsdr;
use oxide_config::clap::Parser;
use oxide_config::OxideInput;
use oxide_logging::SetupLogging;
use oxide_rtlsdr::RtlSdr;
use oxide_scanner;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let args: OxideInput = OxideInput::parse();
    args.logging.enable_logging();
    debug!(
        "Starting ACARS Oxide with the following options: {:#?}",
        args
    );

    // Create a vector of all configured RTLSDR devices

    let mut rtlsdr = vec![];

    match args.sdr1serial {
        Some(serial) => {
            let ppm = args.sdr1ppm.unwrap_or(0);
            let gain = args.sdr1gain.unwrap_or(0);
            let bias_tee = args.sdr1biastee.unwrap_or(false);
            let rtl_mult = args.sdr1mult.unwrap_or(160);
            let frequencies = args.sdr1freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR1 configured")
        }
    }

    match args.sdr2serial {
        Some(serial) => {
            let ppm = args.sdr2ppm.unwrap_or(0);
            let gain = args.sdr2gain.unwrap_or(0);
            let bias_tee = args.sdr2biastee.unwrap_or(false);
            let rtl_mult = args.sdr2mult.unwrap_or(160);
            let frequencies = args.sdr2freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR2 configured")
        }
    }

    match args.sdr3serial {
        Some(serial) => {
            let ppm = args.sdr3ppm.unwrap_or(0);
            let gain = args.sdr3gain.unwrap_or(0);
            let bias_tee = args.sdr3biastee.unwrap_or(false);
            let rtl_mult = args.sdr3mult.unwrap_or(160);
            let frequencies = args.sdr3freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR3 configured")
        }
    }

    match args.sdr4serial {
        Some(serial) => {
            let ppm = args.sdr4ppm.unwrap_or(0);
            let gain = args.sdr4gain.unwrap_or(0);
            let bias_tee = args.sdr4biastee.unwrap_or(false);
            let rtl_mult = args.sdr4mult.unwrap_or(160);
            let frequencies = args.sdr4freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR4 configured")
        }
    }

    match args.sdr5serial {
        Some(serial) => {
            let ppm = args.sdr5ppm.unwrap_or(0);
            let gain = args.sdr5gain.unwrap_or(0);
            let bias_tee = args.sdr5biastee.unwrap_or(false);
            let rtl_mult = args.sdr5mult.unwrap_or(160);
            let frequencies = args.sdr5freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR5 configured")
        }
    }

    match args.sdr6serial {
        Some(serial) => {
            let ppm = args.sdr6ppm.unwrap_or(0);
            let gain = args.sdr6gain.unwrap_or(0);
            let bias_tee = args.sdr6biastee.unwrap_or(false);
            let rtl_mult = args.sdr6mult.unwrap_or(160);
            let frequencies = args.sdr6freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR6 configured")
        }
    }

    match args.sdr7serial {
        Some(serial) => {
            let ppm = args.sdr7ppm.unwrap_or(0);
            let gain = args.sdr7gain.unwrap_or(0);
            let bias_tee = args.sdr7biastee.unwrap_or(false);
            let rtl_mult = args.sdr7mult.unwrap_or(160);
            let frequencies = args.sdr7freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR7 configured")
        }
    }

    match args.sdr8serial {
        Some(serial) => {
            let ppm = args.sdr8ppm.unwrap_or(0);
            let gain = args.sdr8gain.unwrap_or(0);
            let bias_tee = args.sdr8biastee.unwrap_or(false);
            let rtl_mult = args.sdr8mult.unwrap_or(160);
            let frequencies = args.sdr8freqs.unwrap_or(vec![]);

            let sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR8 configured")
        }
    }

    let scanner = oxide_scanner::OxideScanner::new(rtlsdr);
    scanner.run().await;

    trace!("Starting the sleep loop");

    loop {
        sleep(Duration::from_millis(100)).await;
    }
}
