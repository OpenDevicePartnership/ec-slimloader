#![no_std]

pub mod lifecycle_provisioning;

use ec_slimloader_mcxa::lifecycle::NbootLifecycleState;
pub use lifecycle_provisioning::*;

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Develop {}
    impl Sealed for super::Develop2 {}
    impl Sealed for super::InField {}
    impl Sealed for super::InFieldLocked {}
    impl Sealed for super::OemFieldReturn {}
    impl Sealed for super::FailureAnalysis {}
    impl Sealed for super::Bricked {}
}

pub struct Develop;
pub struct Develop2;
pub struct InField;
pub struct InFieldLocked;
pub struct OemFieldReturn;
pub struct FailureAnalysis;
pub struct Bricked;

/// Compile-time proof that advancing from `Self` to `Next` is a valid transition.
/// Only the explicit `impl` blocks below are permitted.
pub trait CanAdvanceTo<Next>: sealed::Sealed {}

impl CanAdvanceTo<Develop2> for Develop {}
impl CanAdvanceTo<InField> for Develop {}
impl CanAdvanceTo<InField> for Develop2 {}
impl CanAdvanceTo<InFieldLocked> for Develop2 {}
impl CanAdvanceTo<InFieldLocked> for InField {}
impl CanAdvanceTo<InField> for InFieldLocked {}
impl CanAdvanceTo<OemFieldReturn> for InField {}
impl CanAdvanceTo<OemFieldReturn> for InFieldLocked {}
impl CanAdvanceTo<FailureAnalysis> for OemFieldReturn {}
impl CanAdvanceTo<Bricked> for Develop2 {}
impl CanAdvanceTo<Bricked> for InFieldLocked {}
impl CanAdvanceTo<Bricked> for InField {}
impl CanAdvanceTo<Bricked> for OemFieldReturn {}
impl CanAdvanceTo<Bricked> for FailureAnalysis {}

pub trait LifecycleState: sealed::Sealed {
    const RUNTIME_VALUE: NbootLifecycleState;
}

impl LifecycleState for Develop {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::Develop;
}
impl LifecycleState for Develop2 {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::Develop2;
}
impl LifecycleState for InField {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::InField;
}
impl LifecycleState for InFieldLocked {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::InFieldLocked;
}
impl LifecycleState for OemFieldReturn {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::OemFieldReturn;
}
impl LifecycleState for FailureAnalysis {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::FailureAnalysis;
}
impl LifecycleState for Bricked {
    const RUNTIME_VALUE: NbootLifecycleState = NbootLifecycleState::Bricked;
}
