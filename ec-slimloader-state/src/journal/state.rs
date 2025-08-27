use num_enum::{IntoPrimitive, TryFromPrimitive};

pub const MAX_SLOT_COUNT: usize = 0b111;

const CRC: crc::Crc<u8> = crc::Crc::<u8>::new(&crc::CRC_8_OPENSAFETY);

/// Image slot ID.
///
/// Valid values from 0x00 to 0x06.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Slot(u8);

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TooManyBits;

impl TryFrom<u8> for Slot {
    type Error = TooManyBits;

    fn try_from(val: u8) -> Result<Slot, Self::Error> {
        if val >= MAX_SLOT_COUNT as u8 {
            Err(TooManyBits)
        } else {
            Ok(Slot(val))
        }
    }
}

impl From<Slot> for u8 {
    fn from(val: Slot) -> Self {
        val.0
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ParseResult {
    /// Nor flash entry yet to be written.
    Unset,
    /// State is an invalid value.
    Invalid,
}

/// Boot process status as stored in [State] as a 2-bit field.
///
/// The enum values are assigned such that bits can be dropped for the happy flow,
/// ensuring minimal wear on the storage.
#[derive(Debug, PartialEq, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Status {
    /// Initial attempt at booting the target image.
    Initial = 3,
    /// Bootloader marks that the initial attempt at bootloading has started.
    Attempting = 2,
    /// Bootloader has encountered in `Attempting`, meaning that the application failed to `Confirm`.
    ///
    /// Or the application image did not pass verification and was never tried.
    Failed = 1,
    /// Application has marked the boot to be successful, and will boot it in the future.
    Confirmed = 0,
}

/// State record as stored in the State boot journal.
///
/// Care must be taken that `0xffff` is an invalid value,
/// as that is the typical value used by an empty NOR flash cell.
///
/// We ensure this by disallowing Slot value 0b111.
#[derive(PartialEq, Clone, Copy)]
pub struct State([u8; 2]);

impl State {
    pub const fn new(status: Status, target: Slot, backup: Slot) -> Self {
        let mut data = 0u8;
        data |= (status as u8) << 6;
        data |= backup.0 << 3;
        data |= target.0;

        let crc = CRC.checksum(&[data]);

        Self([data, crc])
    }

    pub fn try_new(data: [u8; 2]) -> Result<Self, ParseResult> {
        if data == [0xff, 0xff] {
            return Err(ParseResult::Unset);
        }

        if Self::try_target(data[0]).is_none() || Self::try_backup(data[0]).is_none() {
            return Err(ParseResult::Invalid);
        }

        if !Self::check_crc(data[1], data[0]) {
            return Err(ParseResult::Invalid);
        }

        Ok(State(data))
    }

    pub fn as_bytes(&self) -> [u8; 2] {
        self.0
    }

    fn check_crc(crc: u8, data: u8) -> bool {
        crc == CRC.checksum(&[data])
    }

    pub fn status(&self) -> Status {
        // Note(unsafe): we are sure that any 2-bit u8 is a valid Status.
        unsafe { Status::try_from_primitive(self.0[0] >> 6).unwrap_unchecked() }
    }

    pub fn with_status(&self, status: Status) -> Self {
        Self::new(status, self.target(), self.backup())
    }

    fn try_target(val: u8) -> Option<Slot> {
        Slot::try_from(val & 0b111).ok()
    }

    pub fn target(&self) -> Slot {
        // If Self exists, Slot must be valid.
        unsafe { State::try_target(self.0[0]).unwrap_unchecked() }
    }

    fn try_backup(val: u8) -> Option<Slot> {
        Slot::try_from((val >> 3) & 0b111).ok()
    }

    pub fn backup(&self) -> Slot {
        // If Self exists, Slot must be valid.
        unsafe { State::try_backup(self.0[0]).unwrap_unchecked() }
    }
}

impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("State")
            .field("status", &self.status())
            .field("target", &self.target())
            .field("backup", &self.backup())
            .finish()
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for State {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(
            f,
            "State {{ status: {}, target: {}, backup: {} }}",
            self.status(),
            self.target(),
            self.backup()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test whether we can construct only valid [Slot] values.
    #[test]
    fn slot_construction() {
        for i in 0b0..0b111u8 {
            assert!(Slot::try_from(i).is_ok());
        }

        for i in 0b111..=0xffu8 {
            assert!(Slot::try_from(i).is_err());
        }
    }

    /// Construct all possible [State] values and test whether we can get fields back out again.
    #[test]
    fn state_validity_content() {
        // Test all possible states.
        for status in [Status::Initial, Status::Attempting, Status::Confirmed, Status::Failed] {
            for i in 0b0..0b111u8 {
                let slot_a = Slot::try_from(i).unwrap();

                for j in 0b0..0b111u8 {
                    let slot_b = Slot::try_from(j).unwrap();

                    let state = State::new(status, slot_b, slot_a);
                    assert_eq!(state.status(), status);
                    assert_eq!(state.target(), slot_b);
                    assert_eq!(state.backup(), slot_a);
                }
            }
        }
    }

    /// Try a few handpicked [State] values and assert Crc value.
    #[test]
    fn state_validity_crc() {
        let slot_a = Slot::try_from(1).unwrap();
        let slot_b = Slot::try_from(2).unwrap();

        let state = State::new(Status::Initial, slot_b, slot_a);
        assert_eq!(state.0[1], 12); // Crc
        let state = State::new(Status::Attempting, slot_b, slot_a);
        assert_eq!(state.0[1], 234); // Crc
        let state = State::new(Status::Confirmed, slot_b, slot_a);
        assert_eq!(state.0[1], 9); // Crc
        let state = State::new(Status::Failed, slot_b, slot_a);
        assert_eq!(state.0[1], 239); // Crc
    }
}
