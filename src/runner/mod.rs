//! Worker orchestration and trusted shell execution.
pub mod executor;
mod update;
mod worker;

pub use worker::{UPDATE_EXIT_CODE, Worker, WorkerExit};
