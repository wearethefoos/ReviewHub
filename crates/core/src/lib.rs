//! ReviewHub's shared logic: report formats and validation, the PR comment
//! format, GitHub trust rules, diff parsing and focus scoring. Used by the CLI
//! natively and by the browser extension through WebAssembly.

pub mod classify;
pub mod diff;
pub mod focus;
pub mod github;
pub mod lcov;
pub mod model;
pub mod reports;
pub mod secrets;
