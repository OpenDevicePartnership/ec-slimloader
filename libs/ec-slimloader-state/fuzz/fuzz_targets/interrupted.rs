#![no_main]

extern crate libfuzzer_sys;
extern crate std;

use arbitrary::Arbitrary;
use ec_slimloader_state::{
    flash::{
        self,
        mock::{MockFlashBase, MockFlashError::EarlyShutoff},
        FlashJournal,
    },
    state::State,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: Input| fuzz(input.states, input.fail_at));

#[derive(Debug)]
struct Input {
    /// Set of consecutive states to write to disk.
    pub states: Vec<State>,

    /// Byte number to fail at when doing disk operations.
    pub fail_at: usize,
}

impl<'a> Arbitrary<'a> for Input {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let states: Vec<State> = Arbitrary::arbitrary(u)?;
        let fail_at = u.int_in_range(0..=states.len() * 2)?;
        Ok(Input { states, fail_at })
    }
}

const PAGES: usize = 4;
const WORD_SIZE: usize = 2;
const WORDS_PER_PAGE: usize = 16;

/// Tests for 'any input disk, with valid or invalid data, does not cause a crash'.
fn fuzz(states: Vec<State>, fail_at: usize) {
    let mut flash = MockFlashBase::<PAGES, WORD_SIZE, WORDS_PER_PAGE>::new(Some(fail_at as u32), false);
    let mut states = states.into_iter().peekable();

    futures::executor::block_on(async {
        let mut journal = FlashJournal::new::<4>(&mut flash).await.unwrap();

        let mut prev_state = None;
        while let Some(new_state) = states.peek() {
            match journal.set::<4>(new_state).await {
                Ok(_) => {
                    assert_eq!(journal.get(), Some(new_state));
                }
                Err(flash::Error::Other(EarlyShutoff(_, _))) => {
                    drop(journal);
                    flash.remove_shutoff();
                    journal = FlashJournal::new::<4>(&mut flash).await.unwrap();
                    let old_state = journal.get();

                    if old_state == prev_state.as_ref() {
                        // Old state was at least kept, even though new state was not persisted.
                        continue;
                    } else if old_state == Some(new_state) {
                        // New state was successfully persisted, even though we had a shutoff (probably stopped during some bookkeeping?).
                    } else {
                        panic!("State not maintained or persisted");
                    }
                    break;
                }
                Err(e) => panic!("Unexpected error {:?}", e),
            }

            prev_state = states.next(); // Successfully persisted, drop the state.
        }
    });
}
