#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use ec_slimloader::BootError;
use embassy_mcxa::rom::{FlashError, FlexspiError, KbError, NbootError, SpiFlashError};

pub(crate) fn map_flash_status_to_boot_error(status: FlashError) -> BootError {
    match status {
        FlashError::InvalidArgument
        | FlashError::AlignmentError
        | FlashError::AddressError
        | FlashError::SizeError
        | FlashError::RegionExecuteOnly
        | FlashError::ReadOnlyProperty => BootError::MemoryRegion,
        FlashError::EccError | FlashError::CompareError | FlashError::CommandFailure => BootError::Integrity,
        FlashError::EraseKeyError
        | FlashError::UnknownProperty
        | FlashError::CommandNotSupported
        | FlashError::InvalidPropertyValue
        | FlashError::InvalidWaitStateCycles => BootError::Markers,
        _ => BootError::IO,
    }
}

pub(crate) fn map_spiflash_status_to_boot_error(status: SpiFlashError) -> BootError {
    match status {
        SpiFlashError::Fail => BootError::IO,
        _ => BootError::IO,
    }
}

pub(crate) fn map_flexspi_status_to_boot_error(status: FlexspiError) -> BootError {
    match status {
        FlexspiError::InvalidArgument
        | FlexspiError::InvalidSequence
        | FlexspiError::SfdpNotFound
        | FlexspiError::UnsupportedSfdpVersion => BootError::Markers,
        FlexspiError::WriteAlignmentError => BootError::MemoryRegion,
        FlexspiError::ProgramFail
        | FlexspiError::EraseSectorFail
        | FlexspiError::EraseAllFail
        | FlexspiError::CommandFailure
        | FlexspiError::DtrReadDummyProbeFailed => BootError::Integrity,
        FlexspiError::Fail
        | FlexspiError::SequenceExecutionTimeout
        | FlexspiError::DeviceTimeout
        | FlexspiError::WaitTimeout
        | FlexspiError::FlashNotFound => BootError::IO,
        _ => BootError::IO,
    }
}

pub(crate) fn map_nboot_status_to_boot_error(status: NbootError) -> BootError {
    match status {
        NbootError::OperationDisallowed => BootError::MemoryRegion,
        NbootError::InvalidArgument => BootError::Markers,
        NbootError::KeyNotAvailable => BootError::Authenticate,
        NbootError::Fail => BootError::IO,
        _ => BootError::Authenticate,
    }
}

pub(crate) fn map_kb_status_to_boot_error(status: KbError) -> BootError {
    match status {
        KbError::InvalidArgument | KbError::RomApiBufferSizeNotEnough | KbError::RomApiInvalidBuffer => {
            BootError::Markers
        }
        KbError::Fail => BootError::IO,
        KbError::RomLdrRollbackBlocked => BootError::Markers,
        // Data underrun / pending-jump are usually flow control for streaming loaders,
        // but if surfaced as an error, treat as I/O.
        KbError::RomLdrDataUnderrun | KbError::RomLdrJumpReturned | KbError::RomLdrPendingJumpCommand => BootError::IO,
        _ => BootError::IO,
    }
}
