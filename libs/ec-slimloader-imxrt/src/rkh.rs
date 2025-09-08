use embassy_imxrt::{hashcrypt::Hashcrypt, peripherals::HASHCRYPT, Peri};

/// A Root Key Hash as lives in the Certificate Block at the end.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rkh(pub [u8; 32]);

/// A Root Key Table Hash.
#[derive(PartialEq, Debug)]
#[repr(C)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rkth(pub [u8; 32]);

impl Rkth {
    pub fn from_rkhs(rkhs: &[Rkh; 4], hashcrypt: Peri<HASHCRYPT>) -> Self {
        // Safety: Rkh's will be at least as aligned as u8's.
        let rkhs = unsafe {
            core::slice::from_raw_parts(rkhs.as_ptr() as *const u8, rkhs.len() * core::mem::size_of::<Rkh>())
        };
        let mut hashcrypt = Hashcrypt::new_blocking(hashcrypt);

        let mut result = [0u8; 32];
        hashcrypt.new_sha256().hash(rkhs, &mut result);
        Rkth(result)
    }
}
