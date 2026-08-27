#![allow(non_upper_case_globals)]
#![allow(dead_code)]

use ec_slimloader::BootError;

const KSTATUS_SUCCESS: u32 = 0;
const KSTATUS_FAIL: u32 = 1;
const KSTATUS_INVALID_ARGUMENT: u32 = 4;

const KSTATUS_FLASH_SUCCESS: u32 = 0;
const KSTATUS_FLASH_INVALID_ARGUMENT: u32 = 4;
const KSTATUS_FLASH_ALIGNMENT_ERROR: u32 = 101;
const KSTATUS_FLASH_ADDRESS_ERROR: u32 = 102;
const KSTATUS_FLASH_SIZE_ERROR: u32 = 100;
const KSTATUS_FLASH_COMMAND_FAILURE: u32 = 105;
const KSTATUS_FLASH_UNKNOWN_PROPERTY: u32 = 106;
const KSTATUS_FLASH_ERASE_KEY_ERROR: u32 = 107;
const KSTATUS_FLASH_REGION_EXECUTE_ONLY: u32 = 108;
const KSTATUS_FLASH_COMMAND_NOT_SUPPORTED: u32 = 111;
const KSTATUS_FLASH_READ_ONLY_PROPERTY: u32 = 112;
const KSTATUS_FLASH_INVALID_PROPERTY_VALUE: u32 = 113;
const KSTATUS_FLASH_ECC_ERROR: u32 = 116;
const KSTATUS_FLASH_COMPARE_ERROR: u32 = 117;
const KSTATUS_FLASH_INVALID_WAIT_STATE_CYCLES: u32 = 119;

// SPI flash driver status codes
const KSTATUS_SPIFLASH_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_SPIFLASH_FAIL: u32 = KSTATUS_FAIL;

// FlexSPI flash driver status codes
const KSTATUS_FLEXSPI_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_FLEXSPI_FAIL: u32 = KSTATUS_FAIL;
const KSTATUS_FLEXSPI_INVALID_ARGUMENT: u32 = KSTATUS_INVALID_ARGUMENT;
const KSTATUS_FLEXSPI_SEQUENCE_EXECUTION_TIMEOUT: u32 = 6000;
const KSTATUS_FLEXSPI_INVALID_SEQUENCE: u32 = 6001;
const KSTATUS_FLEXSPI_DEVICE_TIMEOUT: u32 = 6002;

const KSTATUS_FLEXSPINOR_PROGRAM_FAIL: u32 = 20100;
const KSTATUS_FLEXSPINOR_ERASE_SECTOR_FAIL: u32 = 20101;
const KSTATUS_FLEXSPINOR_ERASE_ALL_FAIL: u32 = 20102;
const KSTATUS_FLEXSPINOR_WAIT_TIMEOUT: u32 = 20103;
const KSTATUS_FLEXSPINOR_WRITE_ALIGNMENT_ERROR: u32 = 20105;
const KSTATUS_FLEXSPINOR_COMMAND_FAILURE: u32 = 20106;
const KSTATUS_FLEXSPINOR_SFDP_NOT_FOUND: u32 = 20107;
const KSTATUS_FLEXSPINOR_UNSUPPORTED_SFDP_VERSION: u32 = 20108;
const KSTATUS_FLEXSPINOR_FLASH_NOT_FOUND: u32 = 20109;
const KSTATUS_FLEXSPINOR_DTR_READ_DUMMY_PROBE_FAILED: u32 = 20110;

const KSTATUS_NBOOT_SUCCESS: u32 = 0x5A5A_5A5A;
const KSTATUS_NBOOT_FAIL: u32 = 0x5A5A_A5A5;
const KSTATUS_NBOOT_INVALID_ARGUMENT: u32 = 0x5A5A_A5F0;

// NBOOT API status codes (MCXA ROM, Table 46 / 9.2.5.11)
// These are returned by APIs such as `nboot_mem_crypt_range_checker`.
const KNBOOT_OPERATION_ALLOWED: u32 = 0x3C5A_33CC;
const KNBOOT_OPERATION_DISALLOWED: u32 = 0x5AA5_CC33;
const KSTATUS_NBOOT_KEY_NOT_AVAILABLE: u32 = 0x5A5A_A5E6;

const KSTATUS_ROMLDR_DATA_UNDERRUN: u32 = 10109;
const KSTATUS_ROMLDR_JUMP_RETURNED: u32 = 10110;
const KSTATUS_ROMLDR_ROLLBACK_BLOCKED: u32 = 10115;
const KSTATUS_ROMLDR_PENDING_JUMP_COMMAND: u32 = 10119;

// ROM API status codes
const KSTATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH: u32 = 10802;
const KSTATUS_ROM_API_INVALID_BUFFER: u32 = 10803;

// KBoot (KB) status codes (Table 35); KB reuses generic/ROM loader/ROM API status space; these aliases make callsites clearer.
const KSTATUS_KB_SUCCESS: u32 = KSTATUS_SUCCESS;
const KSTATUS_KB_FAIL: u32 = KSTATUS_FAIL;
const KSTATUS_KB_INVALID_ARGUMENT: u32 = KSTATUS_INVALID_ARGUMENT;

const KSTATUS_KB_ROMLDR_DATA_UNDERRUN: u32 = KSTATUS_ROMLDR_DATA_UNDERRUN;
const KSTATUS_KB_ROMLDR_JUMP_RETURNED: u32 = KSTATUS_ROMLDR_JUMP_RETURNED;
const KSTATUS_KB_ROMLDR_ROLLBACK_BLOCKED: u32 = KSTATUS_ROMLDR_ROLLBACK_BLOCKED;
const KSTATUS_KB_ROMLDR_PENDING_JUMP_COMMAND: u32 = KSTATUS_ROMLDR_PENDING_JUMP_COMMAND;

const KSTATUS_KB_BUFFER_SIZE_NOT_ENOUGH: u32 = KSTATUS_ROM_API_BUFFER_SIZE_NOT_ENOUGH;
const KSTATUS_KB_INVALID_BUFFER: u32 = KSTATUS_ROM_API_INVALID_BUFFER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u32)]
pub enum FlashStatus {
    Success = KSTATUS_FLASH_SUCCESS,
    InvalidArgument = KSTATUS_FLASH_INVALID_ARGUMENT,
    AlignmentError = KSTATUS_FLASH_ALIGNMENT_ERROR,
    AddressError = KSTATUS_FLASH_ADDRESS_ERROR,
    SizeError = KSTATUS_FLASH_SIZE_ERROR,
    CommandFailure = KSTATUS_FLASH_COMMAND_FAILURE,
    UnknownProperty = KSTATUS_FLASH_UNKNOWN_PROPERTY,
    EraseKeyError = KSTATUS_FLASH_ERASE_KEY_ERROR,
    RegionExecuteOnly = KSTATUS_FLASH_REGION_EXECUTE_ONLY,
    CommandNotSupported = KSTATUS_FLASH_COMMAND_NOT_SUPPORTED,
    ReadOnlyProperty = KSTATUS_FLASH_READ_ONLY_PROPERTY,
    InvalidPropertyValue = KSTATUS_FLASH_INVALID_PROPERTY_VALUE,
    EccError = KSTATUS_FLASH_ECC_ERROR,
    CompareError = KSTATUS_FLASH_COMPARE_ERROR,
    InvalidWaitStateCycles = KSTATUS_FLASH_INVALID_WAIT_STATE_CYCLES,
    Unknown(u32) = 0xFFFF_FFFF,
}

impl From<u32> for FlashStatus {
    fn from(raw: u32) -> Self {
        match raw {
            KSTATUS_FLASH_SUCCESS => Self::Success,
            KSTATUS_FLASH_INVALID_ARGUMENT => Self::InvalidArgument,
            KSTATUS_FLASH_ALIGNMENT_ERROR => Self::AlignmentError,
            KSTATUS_FLASH_ADDRESS_ERROR => Self::AddressError,
            KSTATUS_FLASH_SIZE_ERROR => Self::SizeError,
            KSTATUS_FLASH_COMMAND_FAILURE => Self::CommandFailure,
            KSTATUS_FLASH_UNKNOWN_PROPERTY => Self::UnknownProperty,
            KSTATUS_FLASH_ERASE_KEY_ERROR => Self::EraseKeyError,
            KSTATUS_FLASH_REGION_EXECUTE_ONLY => Self::RegionExecuteOnly,
            KSTATUS_FLASH_COMMAND_NOT_SUPPORTED => Self::CommandNotSupported,
            KSTATUS_FLASH_READ_ONLY_PROPERTY => Self::ReadOnlyProperty,
            KSTATUS_FLASH_INVALID_PROPERTY_VALUE => Self::InvalidPropertyValue,
            KSTATUS_FLASH_ECC_ERROR => Self::EccError,
            KSTATUS_FLASH_COMPARE_ERROR => Self::CompareError,
            KSTATUS_FLASH_INVALID_WAIT_STATE_CYCLES => Self::InvalidWaitStateCycles,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum SpiFlashStatus {
    Success = KSTATUS_SPIFLASH_SUCCESS,
    Fail = KSTATUS_SPIFLASH_FAIL,
    Unknown(u32) = 0xFFFF_FFFF,
}

impl From<u32> for SpiFlashStatus {
    fn from(raw: u32) -> Self {
        match raw {
            KSTATUS_SPIFLASH_SUCCESS => Self::Success,
            KSTATUS_SPIFLASH_FAIL => Self::Fail,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum FlexspiStatus {
    Success = KSTATUS_FLEXSPI_SUCCESS,
    Fail = KSTATUS_FLEXSPI_FAIL,
    InvalidArgument = KSTATUS_FLEXSPI_INVALID_ARGUMENT,
    SequenceExecutionTimeout = KSTATUS_FLEXSPI_SEQUENCE_EXECUTION_TIMEOUT,
    InvalidSequence = KSTATUS_FLEXSPI_INVALID_SEQUENCE,
    DeviceTimeout = KSTATUS_FLEXSPI_DEVICE_TIMEOUT,
    ProgramFail = KSTATUS_FLEXSPINOR_PROGRAM_FAIL,
    EraseSectorFail = KSTATUS_FLEXSPINOR_ERASE_SECTOR_FAIL,
    EraseAllFail = KSTATUS_FLEXSPINOR_ERASE_ALL_FAIL,
    WaitTimeout = KSTATUS_FLEXSPINOR_WAIT_TIMEOUT,
    WriteAlignmentError = KSTATUS_FLEXSPINOR_WRITE_ALIGNMENT_ERROR,
    CommandFailure = KSTATUS_FLEXSPINOR_COMMAND_FAILURE,
    SfdpNotFound = KSTATUS_FLEXSPINOR_SFDP_NOT_FOUND,
    UnsupportedSfdpVersion = KSTATUS_FLEXSPINOR_UNSUPPORTED_SFDP_VERSION,
    FlashNotFound = KSTATUS_FLEXSPINOR_FLASH_NOT_FOUND,
    DtrReadDummyProbeFailed = KSTATUS_FLEXSPINOR_DTR_READ_DUMMY_PROBE_FAILED,
    Unknown(u32) = 0xFFFF_FFFF,
}

impl From<u32> for FlexspiStatus {
    fn from(raw: u32) -> Self {
        match raw {
            KSTATUS_FLEXSPI_SUCCESS => Self::Success,
            KSTATUS_FLEXSPI_FAIL => Self::Fail,
            KSTATUS_FLEXSPI_INVALID_ARGUMENT => Self::InvalidArgument,
            KSTATUS_FLEXSPI_SEQUENCE_EXECUTION_TIMEOUT => Self::SequenceExecutionTimeout,
            KSTATUS_FLEXSPI_INVALID_SEQUENCE => Self::InvalidSequence,
            KSTATUS_FLEXSPI_DEVICE_TIMEOUT => Self::DeviceTimeout,
            KSTATUS_FLEXSPINOR_PROGRAM_FAIL => Self::ProgramFail,
            KSTATUS_FLEXSPINOR_ERASE_SECTOR_FAIL => Self::EraseSectorFail,
            KSTATUS_FLEXSPINOR_ERASE_ALL_FAIL => Self::EraseAllFail,
            KSTATUS_FLEXSPINOR_WAIT_TIMEOUT => Self::WaitTimeout,
            KSTATUS_FLEXSPINOR_WRITE_ALIGNMENT_ERROR => Self::WriteAlignmentError,
            KSTATUS_FLEXSPINOR_COMMAND_FAILURE => Self::CommandFailure,
            KSTATUS_FLEXSPINOR_SFDP_NOT_FOUND => Self::SfdpNotFound,
            KSTATUS_FLEXSPINOR_UNSUPPORTED_SFDP_VERSION => Self::UnsupportedSfdpVersion,
            KSTATUS_FLEXSPINOR_FLASH_NOT_FOUND => Self::FlashNotFound,
            KSTATUS_FLEXSPINOR_DTR_READ_DUMMY_PROBE_FAILED => Self::DtrReadDummyProbeFailed,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u32)]
pub enum NbootStatus {
    Success = KSTATUS_NBOOT_SUCCESS,
    Fail = KSTATUS_NBOOT_FAIL,
    InvalidArgument = KSTATUS_NBOOT_INVALID_ARGUMENT,
    OperationAllowed = KNBOOT_OPERATION_ALLOWED,
    OperationDisallowed = KNBOOT_OPERATION_DISALLOWED,
    KeyNotAvailable = KSTATUS_NBOOT_KEY_NOT_AVAILABLE,
    Unknown(u32) = 0xFFFF_FFFF, // Catch-all for unknown status codes, including saturated values.
}

impl From<u64> for NbootStatus {
    fn from(raw: u64) -> Self {
        // The ROM returns a usable 32-bit value; upper 32 bits are likely security related metadata/fault attack protection.
        // TODO: Discuss the FI counter usage with NXP.
        let raw = raw as u32;
        match raw {
            KSTATUS_NBOOT_SUCCESS => Self::Success,
            KSTATUS_NBOOT_FAIL => Self::Fail,
            KSTATUS_NBOOT_INVALID_ARGUMENT => Self::InvalidArgument,
            KNBOOT_OPERATION_ALLOWED => Self::OperationAllowed,
            KNBOOT_OPERATION_DISALLOWED => Self::OperationDisallowed,
            KSTATUS_NBOOT_KEY_NOT_AVAILABLE => Self::KeyNotAvailable,
            other => Self::Unknown(other),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum KbStatus {
    Success = KSTATUS_KB_SUCCESS,
    Fail = KSTATUS_KB_FAIL,
    InvalidArgument = KSTATUS_KB_INVALID_ARGUMENT,
    RomLdrDataUnderrun = KSTATUS_KB_ROMLDR_DATA_UNDERRUN,
    RomLdrJumpReturned = KSTATUS_KB_ROMLDR_JUMP_RETURNED,
    RomLdrRollbackBlocked = KSTATUS_KB_ROMLDR_ROLLBACK_BLOCKED,
    RomLdrPendingJumpCommand = KSTATUS_KB_ROMLDR_PENDING_JUMP_COMMAND,
    RomApiBufferSizeNotEnough = KSTATUS_KB_BUFFER_SIZE_NOT_ENOUGH,
    RomApiInvalidBuffer = KSTATUS_KB_INVALID_BUFFER,
    Unknown(u32) = 0xFFFF_FFFF,
}

impl From<u32> for KbStatus {
    fn from(raw: u32) -> Self {
        match raw {
            KSTATUS_KB_SUCCESS => Self::Success,
            KSTATUS_KB_FAIL => Self::Fail,
            KSTATUS_KB_INVALID_ARGUMENT => Self::InvalidArgument,
            KSTATUS_KB_ROMLDR_DATA_UNDERRUN => Self::RomLdrDataUnderrun,
            KSTATUS_KB_ROMLDR_JUMP_RETURNED => Self::RomLdrJumpReturned,
            KSTATUS_KB_ROMLDR_ROLLBACK_BLOCKED => Self::RomLdrRollbackBlocked,
            KSTATUS_KB_ROMLDR_PENDING_JUMP_COMMAND => Self::RomLdrPendingJumpCommand,
            KSTATUS_KB_BUFFER_SIZE_NOT_ENOUGH => Self::RomApiBufferSizeNotEnough,
            KSTATUS_KB_INVALID_BUFFER => Self::RomApiInvalidBuffer,
            other => Self::Unknown(other),
        }
    }
}

#[allow(dead_code)]
pub fn map_flash_status_to_boot_error(status: FlashStatus) -> BootError {
    match status {
        FlashStatus::InvalidArgument
        | FlashStatus::AlignmentError
        | FlashStatus::AddressError
        | FlashStatus::SizeError
        | FlashStatus::RegionExecuteOnly
        | FlashStatus::ReadOnlyProperty => BootError::MemoryRegion,
        FlashStatus::EccError | FlashStatus::CompareError | FlashStatus::CommandFailure => BootError::Integrity,
        FlashStatus::EraseKeyError
        | FlashStatus::UnknownProperty
        | FlashStatus::CommandNotSupported
        | FlashStatus::InvalidPropertyValue
        | FlashStatus::InvalidWaitStateCycles => BootError::Markers,
        _ => BootError::IO,
    }
}

#[allow(dead_code)]
pub fn map_spiflash_status_to_boot_error(status: SpiFlashStatus) -> BootError {
    match status {
        SpiFlashStatus::Fail => BootError::IO,
        _ => BootError::IO,
    }
}

#[allow(dead_code)]
pub fn map_flexspi_status_to_boot_error(status: FlexspiStatus) -> BootError {
    match status {
        FlexspiStatus::InvalidArgument
        | FlexspiStatus::InvalidSequence
        | FlexspiStatus::SfdpNotFound
        | FlexspiStatus::UnsupportedSfdpVersion => BootError::Markers,
        FlexspiStatus::WriteAlignmentError => BootError::MemoryRegion,
        FlexspiStatus::ProgramFail
        | FlexspiStatus::EraseSectorFail
        | FlexspiStatus::EraseAllFail
        | FlexspiStatus::CommandFailure
        | FlexspiStatus::DtrReadDummyProbeFailed => BootError::Integrity,
        FlexspiStatus::Fail
        | FlexspiStatus::Success
        | FlexspiStatus::SequenceExecutionTimeout
        | FlexspiStatus::DeviceTimeout
        | FlexspiStatus::WaitTimeout
        | FlexspiStatus::FlashNotFound => BootError::IO,
        _ => BootError::IO,
    }
}

pub fn map_nboot_status_to_boot_error(status: NbootStatus) -> BootError {
    match status {
        NbootStatus::OperationDisallowed => BootError::MemoryRegion,
        NbootStatus::InvalidArgument => BootError::Markers,
        NbootStatus::KeyNotAvailable => BootError::Authenticate,
        NbootStatus::Fail => BootError::IO,
        _ => BootError::Authenticate,
    }
}

#[allow(dead_code)]
pub fn map_kb_status_to_boot_error(status: KbStatus) -> BootError {
    match status {
        KbStatus::InvalidArgument | KbStatus::RomApiBufferSizeNotEnough | KbStatus::RomApiInvalidBuffer => {
            BootError::Markers
        }
        KbStatus::Fail => BootError::IO,
        KbStatus::RomLdrRollbackBlocked => BootError::Markers,
        // Data underrun / pending-jump are usually flow control for streaming loaders,
        // but if surfaced as an error, treat as I/O.
        KbStatus::RomLdrDataUnderrun | KbStatus::RomLdrJumpReturned | KbStatus::RomLdrPendingJumpCommand => {
            BootError::IO
        }
        _ => BootError::IO,
    }
}
