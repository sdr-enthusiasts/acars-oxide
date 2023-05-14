#[macro_use]
extern crate log;
extern crate oxide_rtlsdr;
use oxide_config::clap::Parser;
use oxide_config::OxideInput;
use oxide_logging::SetupLogging;
use oxide_rtlsdr::RtlSdr;

#[tokio::main]
async fn main() {
    let args: OxideInput = OxideInput::parse();
    args.logging.enable_logging();
    debug!(
        "Starting ACARS Oxide with the following options: {:?}",
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

            let mut sdr = RtlSdr::new(serial, ppm, gain, bias_tee, rtl_mult, frequencies);
            sdr.open_sdr();

            rtlsdr.push(sdr);
        }
        None => {
            trace!("No SDR1 configured")
        }
    }
}
