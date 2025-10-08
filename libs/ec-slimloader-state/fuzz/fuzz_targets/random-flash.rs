#![no_main]

extern crate libfuzzer_sys;
extern crate std;

use arbitrary::Arbitrary;
use ec_slimloader_state::{
    flash::{mock::MockFlashBase, FlashJournal},
    state::State,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|input: Input<'_>| fuzz(input.data, input.new_state));

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    pub data: &'a [u8],
    pub new_state: State,
}

const PAGES: usize = 4;
const WORD_SIZE: usize = 2;
const WORDS_PER_PAGE: usize = 16;

/// Tests for 'any input disk, with valid or invalid data, does not cause a crash'.
fn fuzz(random_data: &[u8], new_state: State) {
    let mut flash = MockFlashBase::<PAGES, WORD_SIZE, WORDS_PER_PAGE>::new(None, false);

    let len = random_data.len().min(flash.as_bytes().len());
    flash.as_bytes_mut()[..len].copy_from_slice(&random_data[..len]);

    futures::executor::block_on(async {
        // Instantiation should never crash. (error is only if there are not enough pages)
        let mut journal = FlashJournal::new::<4>(&mut flash).await.unwrap();

        // Finally try to update the state.
        journal.set::<4>(&new_state).await.unwrap();
        assert_eq!(journal.get(), Some(&new_state));
    });
}
