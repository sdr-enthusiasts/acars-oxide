#[macro_use]
extern crate log;

use oxide_rtlsdr::RtlSdr;

pub struct OxideScanner {
    sdrs: Vec<RtlSdr>,
}

impl OxideScanner {
    pub fn new(sdrs: Vec<RtlSdr>) -> OxideScanner {
        OxideScanner { sdrs: sdrs }
    }

    pub async fn run(mut self) {
        for sdr in self.sdrs.iter_mut() {
            info!("{} Opening SDR", sdr.get_serial());
            sdr.open_sdr();
        }

        for sdr in self.sdrs {
            info!("{} Starting SDR", sdr.get_serial());
            tokio::spawn(async move {
                sdr.read_samples().await;
            });
        }
    }
}
