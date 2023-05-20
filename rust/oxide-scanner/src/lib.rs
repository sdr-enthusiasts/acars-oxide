#[macro_use]
extern crate log;

use oxide_rtlsdr::RtlSdr;

pub struct OxideScanner {
    sdrs: Vec<RtlSdr>,
}

impl OxideScanner {
    pub fn new(sdrs: Vec<RtlSdr>) -> OxideScanner {
        OxideScanner { sdrs }
    }

    pub async fn run(self) {
        let mut valid_sdrs: u8 = 0;
        for mut sdr in self.sdrs.into_iter() {
            info!("[OXIDE SCANNER] Opening SDR {}", sdr.get_serial());
            match sdr.open_sdr() {
                Ok(_) => {
                    valid_sdrs += 1;
                    info!("[OXIDE SCANNER] SDR {} opened", sdr.get_serial());
                    tokio::spawn(async move {
                        sdr.read_samples().await;
                    });
                }
                Err(e) => {
                    error!(
                        "[OXIDE SCANNER] Failed to open SDR {}: {}",
                        sdr.get_serial(),
                        e
                    );
                }
            }
        }

        assert!(valid_sdrs > 0, "No valid SDRs found. Exiting program.")
    }
}
