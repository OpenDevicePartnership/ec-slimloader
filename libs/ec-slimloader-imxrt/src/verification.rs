use defmt_or_log::{error, info, unwrap, warn};
use ec_slimloader::BootError;
use embassy_imxrt::hashcrypt::Hashcrypt;
use embassy_imxrt::peripherals::HASHCRYPT;
use embassy_imxrt::Peri;
use imxrt_rom::otp::Otp;
use imxrt_rom::registers::field_sets::Rkth;
use imxrt_rom::registers::{OtpFuses, SecureBoot, ShadowRegisters};

use crate::mbi::Ivt;
use crate::{CheckImage, Imxrt, ImxrtConfig};

// TODO determine clock frequency from HAL.
const SYSTEM_CORE_CLOCK_HZ: u32 = (5 * 1000 * 1000) / 2;

/// A Root Key Hash as lives in the Certificate Block at the end.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rkh(pub [u8; 32]);

impl Rkh {
    pub fn to_rkth(rkhs: &[Rkh; 4], hashcrypt: Peri<HASHCRYPT>) -> Rkth {
        // Safety: Rkh's will be at least as aligned as u8's.
        let rkhs = unsafe {
            core::slice::from_raw_parts(rkhs.as_ptr() as *const u8, rkhs.len() * core::mem::size_of::<Rkh>())
        };
        let mut hashcrypt = Hashcrypt::new_blocking(hashcrypt);

        let mut result = [0u8; 32];
        hashcrypt.new_sha256().hash(rkhs, &mut result);
        Rkth::from(result)
    }

    pub fn read_all_from_slice(data: &[u8]) -> Option<[Rkh; 4]> {
        if data.len() < core::mem::size_of::<[Rkh; 4]>() {
            return None;
        }

        Some(unsafe { (data.as_ptr() as *const [Rkh; 4]).read_unaligned() })
    }
}

fn rkth(rkth: Rkth) -> [u8; 32] {
    rkth.into()
}

impl<C: ImxrtConfig> CheckImage for Imxrt<C> {
    fn check_image(&mut self, ram_ivt: &Ivt) -> Result<(), BootError> {
        // Compute RKTH from image.
        let image_rkth = {
            // Safety: whilst we do not know if the image is valid by itself,
            // this slice at least is what we just copied. (should be identical to target_slice)

            use crate::mbi::CertificateBlockHeader;
            let ram_image_slice =
                unsafe { core::slice::from_raw_parts(ram_ivt.target_ptr as *const u8, ram_ivt.image_len) };
            let cert_block_header_offset = ram_ivt.header_offset as usize;

            // Fetch certificate block
            let Some(cert_block_header) =
                CertificateBlockHeader::read_from_slice(&ram_image_slice[cert_block_header_offset..])
            else {
                return Err(BootError::TooLarge);
            };

            if cert_block_header.header_length != 0x20 {
                warn!("Certificate block header is not expected length");
            }

            let rkhs_offset = cert_block_header_offset
                + cert_block_header.header_length as usize
                + cert_block_header.certificate_table_length as usize;

            let Some(rkhs) = Rkh::read_all_from_slice(&ram_image_slice[rkhs_offset..]) else {
                return Err(BootError::TooLarge);
            };

            Rkh::to_rkth(&rkhs, self.hashcrypt.reborrow())
        };

        info!("RKTH (image) {:x}", rkth(image_rkth));

        let mut shadow = ShadowRegisters::new();

        {
            info!("Boot0 (shadow) {}", unwrap!(shadow.boot_cfg_0().read()));
            info!("Boot1 (shadow) {}", unwrap!(shadow.boot_cfg_1().read()));
            info!("RKTH (shadow) {:x}", rkth(unwrap!(shadow.rkth().read())));
        }

        // Reload shadow registers.
        {
            let mut otp = Otp::init(SYSTEM_CORE_CLOCK_HZ);
            {
                let mut fuses = OtpFuses::readonly(&mut otp);
                info!("Boot0 (fuse): {}", unwrap!(fuses.boot_cfg_0().read()));
                info!("Boot1 (fuse): {}", unwrap!(fuses.boot_cfg_1().read()));
                info!("RKTH (fuse): {:x}", rkth(unwrap!(fuses.rkth().read())));
            }
            unwrap!(otp.reload_shadow());
            info!("Shadow registers reloaded from fuses");
        }

        // Fix for EVK without fuses.
        #[cfg(feature = "mimxrt685s-evk")]
        {
            // Configure the EVK NOR flash @ port 2, pin 12 to be reset on a system reset.
            unwrap!(shadow.boot_cfg_1().modify(|w| {
                w.set_qspi_reset_pin_enable(true);
                w.set_qspi_reset_pin_port(2);
                w.set_qspi_reset_pin_num(12);
            }));
        }

        {
            info!("Boot0 (shadow reloaded) {}", unwrap!(shadow.boot_cfg_0().read()));
            info!("Boot1 (shadow reloaded) {}", unwrap!(shadow.boot_cfg_1().read()));
            info!("RKTH (shadow reloaded) {:x}", rkth(unwrap!(shadow.rkth().read())));
        }

        // Whether the hardware is in 'development mode' is dependent on the secure_boot_en bit being asserted.
        let dev_mode = unwrap!(shadow.boot_cfg_0().read()).secure_boot_en() == SecureBoot::Disabled;

        if image_rkth != unwrap!(shadow.rkth().read()) {
            if dev_mode {
                // If no SECURE_BOOT fuse set => overwrite shadow RKTH with image RKTH
                warn!("Development mode detected, using new image RKTH {:x}", rkth(image_rkth));
                unwrap!(shadow.rkth().write(|w| *w = image_rkth));
            } else {
                // If SECURE_BOOT fuse set => do nothing as skboot_authenticate should be annoyed (perhaps assert afterwards)
                error!("Shadow and image RKTH do not concur, but we call skboot_authenticate in any case");
            }
        } else {
            info!("Shadow and image RKTH concur!")
        }

        info!("Starting authenticate");
        // Call the ROM API to ensure that the image is signed and not broken or tampered with.
        // Note: skboot_authenticate will show false-negatives if your clock jitter is too high.
        // We noticed this with FFROdiv2 and MainClk > 475MHz.
        match imxrt_rom::skboot::skboot_authenticate(ram_ivt.target_ptr, ram_ivt.image_len as u32, None) {
            Ok(()) => {
                info!("Authenticate succeeded!");
                Ok(())
            }
            Err(e) => {
                warn!("Failed to authenticate {:?}", e);
                Err(BootError::Authenticate)
            }
        }
    }
}
