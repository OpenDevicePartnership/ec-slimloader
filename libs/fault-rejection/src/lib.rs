#![cfg_attr(not(test), no_std)]

pub mod fallback;

cfg_select! {
    feature = "force-fallback" => {
        pub use fallback::*;
    }
    all(
        not(feature = "force-fallback"),
        target_arch = "arm",
        any(target_abi = "eabi", target_abi = "eabihf")
    ) => {
        mod arm_eabi;
        pub use arm_eabi::*;
    }
    _ => {
        pub use fallback::*;
    }
}

pub fn test_implementations() {
    assert_eq!(protected_if::<0xAAAAAAAA, _, _, _>(|| 0xAAAAAAAA, || {}), Some(()));
    assert_eq!(protected_if::<0xAAAAAAAA, _, _, _>(|| 0, || {}), None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_implementations_on_host() {
        test_implementations();
    }
}
