#![no_std]
#![doc = include_str!("../README.md")]

pub mod core;
pub mod phy;

#[cfg(feature = "a76xx")]
pub mod a76xx;

pub use core::{Client, Command, Error, Event, ResultCode};

#[cfg(test)]
mod tests;
