//! Temporal quantification for R3-OS.
//!
//! > Duck Guy: “Maybe time's just a construct of human perception, an illusion created by—”
//! >
//! > Tony the Clock: “(alarm sounds) Eh! Eh! Eh! Eh! EH! EH! EH! EH! **EH! EH! EH! EH!**”
//! >
//! > — [*Don't Hug Me I'm Scared 2 - TIME*](https://www.youtube.com/watch?v=vtkGtXtDlQA)
mod duration;
#[allow(clippy::module_inception)]
mod time;
pub use self::{duration::*, time::*};
