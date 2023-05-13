extern crate rtlsdr;
use rtlsdr::RtlSdr;

#[tokio::main]
async fn main() {
    let mut sdr = RtlSdr::new("00013305".to_string());
    sdr.open_sdr();
}
