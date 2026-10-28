//! The Cellworks binary, as a library.
//!
//! The binary is a library plus a thin `main` so the tests in `tests/` can
//! reach the frame body and the argument parser without starting a process or
//! opening a window.

pub mod app;
pub mod cli;
pub mod demo;
pub mod iterate;
pub mod iterate_view;
pub mod opened;
pub mod review;

pub use app::draw;
