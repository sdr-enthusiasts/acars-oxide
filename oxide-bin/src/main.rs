extern crate rtlsdr;
use oxide_config::clap::Parser;
use oxide_config::OxideInput;
use rtlsdr::RtlSdr;

#[tokio::main]
async fn main() {
    let args: OxideInput = OxideInput::parse();
    println!("{:?}", args);
    let mut sdr = RtlSdr::new(
        args.sdr1serial.unwrap(),
        args.sdr1ppm.unwrap(),
        args.sdr1gain.unwrap(),
        args.sdr1bias_tee.unwrap(),
    );
    sdr.open_sdr();
}
