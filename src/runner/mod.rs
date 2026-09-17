//! Worker orchestration and trusted shell execution.
mod client;
pub mod executor;
mod update;
mod worker;

pub use client::CoordinatorClient;
pub use worker::{UPDATE_EXIT_CODE, Worker, WorkerExit};
