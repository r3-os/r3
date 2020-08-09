mod gic_regs;

/// Used by `use_gic!`
#[cfg(target_os = "none")]
pub mod imp;
pub mod cfg;
