use core::fmt::Display;
use core::ops::Range;
use std::vec::Vec;

use embedded_storage_async::nor_flash::{
    ErrorType, MultiwriteNorFlash, NorFlash, NorFlashError, NorFlashErrorKind, ReadNorFlash,
};

/// State of a word in the flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Writable {
    /// Once (can only convert 1 bits to 0
    O,
    /// Never (must be cleared before being writable again)
    N,
}

use Writable::*;

/// Base type for in memory flash that can be used for mocking.
#[derive(Debug, Clone)]
pub struct MockFlashBase<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> {
    writable: Vec<Writable>,
    data: Vec<u8>,
    /// A countdown to shutoff. When some and 0, an early shutoff will happen.
    pub bytes_until_shutoff: Option<u32>,
    /// When true, write buffers have to be aligned
    pub alignment_check: bool,
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> Default
    for MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
    fn default() -> Self {
        Self::new(None, true)
    }
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize>
    MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
    const CAPACITY_WORDS: usize = PAGES * PAGE_WORDS;
    const CAPACITY_BYTES: usize = Self::CAPACITY_WORDS * BYTES_PER_WORD;

    const PAGE_BYTES: usize = PAGE_WORDS * BYTES_PER_WORD;

    /// Create a new flash instance.
    pub fn new(bytes_until_shutoff: Option<u32>, alignment_check: bool) -> Self {
        Self {
            writable: vec![O; Self::CAPACITY_WORDS],
            data: vec![u8::MAX; Self::CAPACITY_BYTES],
            bytes_until_shutoff,
            alignment_check,
        }
    }

    /// Get a reference to the underlying data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the underlying data.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    fn validate_operation(offset: u32, length: usize) -> Result<Range<usize>, MockFlashError> {
        let offset = offset as usize;
        if (offset % Self::READ_SIZE) != 0 {
            Err(MockFlashError::NotAligned)
        } else if offset > Self::CAPACITY_BYTES || offset + length > Self::CAPACITY_BYTES {
            Err(MockFlashError::OutOfBounds)
        } else {
            Ok(offset..(offset + length))
        }
    }

    fn check_shutoff(&mut self, address: u32, operation: Operation) -> Result<(), MockFlashError> {
        if let Some(bytes_until_shutoff) = self.bytes_until_shutoff.as_mut() {
            if let Some(next) = bytes_until_shutoff.checked_sub(1) {
                *bytes_until_shutoff = next;
                Ok(())
            } else {
                self.bytes_until_shutoff = None;
                Err(MockFlashError::EarlyShutoff(address, operation))
            }
        } else {
            Ok(())
        }
    }

    pub fn remove_shutoff(&mut self) {
        self.bytes_until_shutoff = None;
    }
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> ErrorType
    for MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
    type Error = MockFlashError;
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> ReadNorFlash
    for MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
    const READ_SIZE: usize = BYTES_PER_WORD;

    async fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        if bytes.len() % Self::READ_SIZE != 0 {
            panic!("any read must be a multiple of Self::READ_SIZE bytes");
        }

        let range = Self::validate_operation(offset, bytes.len())?;

        bytes.copy_from_slice(&self.as_bytes()[range]);

        Ok(())
    }

    fn capacity(&self) -> usize {
        Self::CAPACITY_BYTES
    }
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> MultiwriteNorFlash
    for MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
}

impl<const PAGES: usize, const BYTES_PER_WORD: usize, const PAGE_WORDS: usize> NorFlash
    for MockFlashBase<PAGES, BYTES_PER_WORD, PAGE_WORDS>
{
    const WRITE_SIZE: usize = BYTES_PER_WORD;

    const ERASE_SIZE: usize = Self::PAGE_BYTES;

    async fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
        let from = from as usize;
        let to = to as usize;

        assert!(from <= to);

        if to > Self::CAPACITY_BYTES {
            return Err(MockFlashError::OutOfBounds);
        }

        if from % Self::PAGE_BYTES != 0 || to % Self::PAGE_BYTES != 0 {
            return Err(MockFlashError::NotAligned);
        }

        for index in from..to {
            self.check_shutoff(index as u32, Operation::Erase)?;
            self.as_bytes_mut()[index] = u8::MAX;

            if index % BYTES_PER_WORD == 0 {
                self.writable[index / BYTES_PER_WORD] = O;
            }
        }

        Ok(())
    }

    async fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        let range = Self::validate_operation(offset, bytes.len())?;

        // Check alignment. Some flash types are strict about the alignment of the input buffer. This ensures
        // that the mock flash is also strict to catch bugs and avoid regressions.
        if self.alignment_check && bytes.as_ptr() as usize % 4 != 0 {
            panic!("write buffer must be aligned to 4 bytes");
        }

        if bytes.len() % Self::WRITE_SIZE != 0 {
            panic!("any write must be a multiple of Self::WRITE_SIZE bytes");
        }

        for (source_word, address) in bytes.chunks_exact(BYTES_PER_WORD).zip(range.step_by(BYTES_PER_WORD)) {
            for (byte_index, byte) in source_word.iter().enumerate() {
                self.check_shutoff((address + byte_index) as u32, Operation::Write)?;

                if byte_index == 0 {
                    let word_writable = &mut self.writable[address / BYTES_PER_WORD];
                    *word_writable = match *word_writable {
                        Writable::O => Writable::N,
                        Writable::N => return Err(MockFlashError::NotWritable(address as u32)),
                    };
                }

                self.as_bytes_mut()[address + byte_index] &= byte;
            }
        }

        Ok(())
    }
}

/// Errors reported by mock flash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockFlashError {
    /// Operation out of bounds.
    OutOfBounds,
    /// Offset or data not aligned.
    NotAligned,
    /// Location not writeable.
    NotWritable(u32),
    /// We got a shutoff
    EarlyShutoff(u32, Operation),
}

impl Display for MockFlashError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl NorFlashError for MockFlashError {
    fn kind(&self) -> NorFlashErrorKind {
        match self {
            MockFlashError::OutOfBounds => NorFlashErrorKind::OutOfBounds,
            MockFlashError::NotAligned => NorFlashErrorKind::NotAligned,
            MockFlashError::NotWritable(_) => NorFlashErrorKind::Other,
            MockFlashError::EarlyShutoff(_, _) => NorFlashErrorKind::Other,
        }
    }
}

#[allow(missing_docs)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Write,
    Erase,
}
