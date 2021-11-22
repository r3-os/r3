use r3::kernel::traits;

/// Indicates a Boost Priority capability.
///
/// This token is returned by
/// [`KernelBoostPriorityExt::BOOST_PRIORITY_CAPABILITY`]. You can also create
/// this directly, but [`KernelBoostPriorityExt::boost_priority`] will panic if
/// Boost Priority isn't actually supported.
pub struct BoostPriorityCapability;

/// Extends system types to add `boost_priority` unconditionally. Whether
/// `boost_priority` is actually supported is controlled by the `priority_boost`
/// feature.
pub trait KernelBoostPriorityExt: traits::KernelBase {
    /// Indicates whether Priority Boost is supported.
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability>;

    /// Enable Priority Boost if it's supported. Panic otherwise.
    #[track_caller]
    fn boost_priority(cap: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError>;
}

#[cfg(not(feature = "priority_boost"))]
impl<T: traits::KernelBase> KernelBoostPriorityExt for T {
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability> = None;

    #[inline]
    #[track_caller]
    fn boost_priority(_: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError> {
        unreachable!("Priority Boost is not supported")
    }
}

#[cfg(feature = "priority_boost")]
impl<T: traits::KernelBase + traits::KernelBoostPriority> KernelBoostPriorityExt for T {
    const BOOST_PRIORITY_CAPABILITY: Option<BoostPriorityCapability> =
        Some(BoostPriorityCapability);

    #[inline]
    #[track_caller]
    fn boost_priority(_: BoostPriorityCapability) -> Result<(), r3::kernel::BoostPriorityError> {
        <Self as traits::KernelBoostPriority>::boost_priority()
    }
}
