#[cfg(any(test, feature = "_test"))]
pub mod mock;

use core::ops::Range;

use embedded_storage_async::nor_flash::NorFlash;

use crate::state::{ParseResult, State};

/// Error describing that the Nvm should have at least two partitions.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error<E> {
    /// A storage medium has been passed that does not contain at least two pages.
    NotEnoughPartitions,

    /// After writing the state a readback does not yield the same state.
    ///
    /// This indicates that something went wrong in the writing process, either on the bus
    /// or in the storage medium itself.
    ReadbackFailed,

    /// The underlying storage medium yielded an error.
    Other(E),
}

impl<E> From<E> for Error<E> {
    fn from(value: E) -> Self {
        Error::Other(value)
    }
}

#[derive(Debug)]
struct StateWithAddr {
    /// Actual value of the [State].
    state: State,
    /// Address of the [State].
    address: usize,
}

#[derive(Default)]
struct Cache {
    /// A copy of the last valid [State] on-disk.
    last_valid_state: Option<StateWithAddr>,

    /// Address of last empty slot for a [State], containing only 0xff bytes.
    first_empty_slot: Option<usize>,
}

/// Bootloader [State] journal backed by Non-Volatile Memory.
pub struct FlashJournal<T> {
    /// Inner flash storage.
    inner: T,
    /// A in-ram cache of the state on disk and where to write the next state to.
    cache: Cache,
}

impl<T: NorFlash> FlashJournal<T> {
    const PAGE_SIZE: usize = T::ERASE_SIZE;

    /// Construct the FlashJournal given a storage device (or a partition).
    ///
    /// Will yield [Error::NotEnoughPartitions] if the partition is not contain at least 2 pages.
    pub async fn new<const N: usize>(mut inner: T) -> Result<Self, Error<T::Error>> {
        if Self::page_count(&inner) < 2 {
            return Err(Error::NotEnoughPartitions);
        }

        let cache = Self::compute_cache::<N>(&mut inner).await?;
        Ok(Self { inner, cache })
    }

    /// Number of pages in the backing storage medium.
    fn page_count(inner: &T) -> usize {
        inner.capacity().div_ceil(Self::PAGE_SIZE)
    }

    /// Convert a memory address in the inner NVM to a page index number.
    fn address_to_page_i(address: u32) -> usize {
        address as usize / Self::PAGE_SIZE
    }

    /// Walk through the entire NVM range, and find the last valid [State] entry
    /// and find the first empty slot of a [State] entry, if any.
    ///
    /// `BLOCK_SIZE` denotes the number of bytes that are read in a single batch
    /// and are analysed, before reading the next block.
    /// A larger block size generally improves performance, and needs to be a non-zero multiple of 2 bytes.
    async fn compute_cache<const BLOCK_SIZE: usize>(inner: &mut T) -> Result<Cache, T::Error> {
        const CHUNK_SIZE: usize = 2;

        defmt_or_log::assert!(BLOCK_SIZE >= CHUNK_SIZE);
        defmt_or_log::assert!(BLOCK_SIZE.is_multiple_of(CHUNK_SIZE));

        let mut buf = [0u8; BLOCK_SIZE];
        let block_count = inner.capacity().div_ceil(BLOCK_SIZE);

        let mut result = Cache::default();
        for block_i in 0..block_count {
            let block_start = block_i * BLOCK_SIZE;
            let block_end = (block_start + BLOCK_SIZE).min(inner.capacity());

            let slice = &mut buf[0..block_end - block_start];
            inner.read(block_start as u32, slice).await?;

            for (chunk_i, chunk) in slice.chunks_exact(CHUNK_SIZE).enumerate() {
                // Note(unsafe): we are using chunks_exact and then cast the slice into the same size array.
                let chunk: [u8; CHUNK_SIZE] = unsafe { chunk.try_into().unwrap_unchecked() };
                let address = block_start + chunk_i * CHUNK_SIZE;
                match State::try_new(chunk) {
                    Ok(state) => {
                        result = Cache {
                            last_valid_state: Some(StateWithAddr { state, address }),
                            first_empty_slot: None, // Reset if any.
                        };
                    }
                    Err(ParseResult::Unset) => {
                        // If not found an empty entry yet, we can record this one as the first one free.
                        if result.first_empty_slot.is_none() {
                            result.first_empty_slot = Some(address);
                        }
                    }
                    Err(ParseResult::Invalid) => {} // Broken.
                }
            }
        }
        Ok(result)
    }

    /// Get the latest [State] contained in the [FlashJournal], if any.
    pub fn get(&self) -> Option<&State> {
        self.cache
            .last_valid_state
            .as_ref()
            .map(|StateWithAddr { state, address: _ }| state)
    }

    /// Erase a range of pages as a single erase instruction to [NorFlash].
    async fn erase_pages(&mut self, page_range: Range<usize>) -> Result<(), T::Error> {
        let start = page_range.start * Self::PAGE_SIZE;
        let end = (page_range.end * Self::PAGE_SIZE).min(self.inner.capacity());
        self.inner.erase(start as u32, end as u32).await
    }

    /// Synchronize the latest [State] to the [FlashJournal].
    pub async fn set<const N: usize>(&mut self, state: &State) -> Result<(), Error<T::Error>> {
        // Check if the current state is identical.
        if self.get() == Some(state) {
            return Ok(());
        }

        // Write the new state somewhere.
        if let Some(first_empty_slot) = self.cache.first_empty_slot {
            // If detected first empty slot, we can write to it as we are [NorFlash] and the empty slot is all `0xff``.
            self.inner.write(first_empty_slot as u32, &state.as_bytes()).await?;
        } else if let Some(last_valid_state) = &self.cache.last_valid_state {
            // If detected no empty slot, we can assume that all pages have been written, or we are in a partially valid state.

            let page_i = Self::address_to_page_i(last_valid_state.address as u32);
            if page_i > 0 {
                // Last valid state is not in the first page, so we can erase it freely. (typical happy flow)
                self.erase_pages(0..1).await?;

                // Write state.
                self.inner.write(0, &state.as_bytes()).await?;

                // Erase rest of pages, and the erasure of the final page will validate our just written state.
                // If this gets interrupted, the last state will remain valid.
                self.erase_pages(1..Self::page_count(&self.inner)).await?;
            } else {
                // Last valid state is in the first page, but the rest of the pages contain no free slot.
                // This edge-case we need to deal with separately.

                // Erase the last pages, which is safe as our last state lives in the first page.
                let second_page_i = 1;
                self.erase_pages(second_page_i..Self::page_count(&self.inner)).await?;

                // Write the state to the first address in the second page, immediately becoming the newest valid state.
                let state_address = second_page_i * Self::PAGE_SIZE;
                self.inner.write(state_address as u32, &state.as_bytes()).await?;
            }
        } else {
            // No state is stored anywhere, and there are no empty slots, clear everything, write.
            self.inner.erase(0, self.inner.capacity() as u32).await?;
            self.inner.write(0, &state.as_bytes()).await?;
        }

        // Re-compute the cache to check if the journal is valid.
        self.cache = Self::compute_cache::<N>(&mut self.inner).await?;

        // Check if the readback is successful.
        if self.get() == Some(state) {
            Ok(())
        } else {
            Err(Error::ReadbackFailed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flash::mock::MockFlashBase;
    use crate::state::{Slot, Status};

    async fn test_journal(nvm: impl NorFlash, assert_empty: bool) -> Option<usize> {
        let mut journal = FlashJournal::new::<4>(nvm).await.unwrap();

        if assert_empty {
            assert!(journal.get().is_none());
        }

        {
            let slot_a = Slot::try_from(1).unwrap();
            let slot_b = Slot::try_from(2).unwrap();

            let state = State::new(Status::Initial, slot_b, slot_a);
            journal.set::<4>(&state).await.unwrap();
            assert_eq!(journal.get(), Some(&state));

            // Re-do the same operation.
            journal.set::<4>(&state).await.unwrap();
            assert_eq!(journal.get(), Some(&state));
        }

        // Write all different kinds of states.
        for status in [Status::Initial, Status::Attempting, Status::Confirmed, Status::Failed] {
            for i in 0b0..0b111u8 {
                let slot_a = Slot::try_from(i).unwrap();

                for j in 0b0..0b111u8 {
                    let slot_b = Slot::try_from(j).unwrap();

                    let state = State::new(status, slot_b, slot_a);
                    journal.set::<4>(&state).await.unwrap();
                    assert_eq!(journal.get(), Some(&state));
                }
            }
        }

        journal.cache.first_empty_slot
    }

    #[test]
    fn journal_normal() {
        let mut mock: MockFlashBase<3, 2, 8> = MockFlashBase::new(None, false);
        embassy_futures::block_on(test_journal(&mut mock, true));
    }

    #[test]
    fn journal_garbage() {
        let mut mock: MockFlashBase<3, 2, 8> = MockFlashBase::new(None, false);
        embassy_futures::block_on(async {
            // Write garbage to pages 1 and 2.
            let bytes = [0xaa; 32];
            mock.write(16, &bytes).await.unwrap();
            let valid_address = test_journal(&mut mock, true).await;

            // Insert broken as we happen to have a valid address with this sequence.
            mock.write(valid_address.unwrap() as u32, &[0xaa, 0xaa]).await.unwrap();

            test_journal(&mut mock, false).await;
        });
    }

    #[test]
    fn journal_realistic() {
        // Use a realistic page count and size.
        let mut mock: MockFlashBase<2, 2, 2048> = MockFlashBase::new(None, false);
        embassy_futures::block_on(async {
            for status in [Status::Initial, Status::Attempting, Status::Confirmed, Status::Failed] {
                for i in 0b0..0b111u8 {
                    let slot_a = Slot::try_from(i).unwrap();

                    for j in 0b0..0b111u8 {
                        let slot_b = Slot::try_from(j).unwrap();

                        let state = State::new(status, slot_b, slot_a);

                        // Practice you re-init the journal every boot and application load.
                        let mut journal = FlashJournal::new::<256>(&mut mock).await.unwrap();
                        journal.set::<4>(&state).await.unwrap();
                        assert_eq!(journal.get(), Some(&state));
                    }
                }
            }
        });
    }
}
