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

    let mut sdr = RtlSdr::new(
        args.sdr1serial.unwrap(),
        args.sdr1ppm.unwrap(),
        args.sdr1gain.unwrap(),
        args.sdr1biastee.unwrap(),
        args.sdr1mult.unwrap(),
        args.sdr1freqs.unwrap(),
    );
    sdr.open_sdr();
}
