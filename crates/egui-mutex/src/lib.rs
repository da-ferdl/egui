//! Simple lightweight mutex implementation for internal egui usage.
//!
//! This replaces the `epaint::mutex` and is like the previous one tailored for internal use in egui
//! where a mutex is strictly needed.
//! Should only be used for short locks (as a guideline, locks should never be held longer than a single frame).
//!
//! Attention: This mutex implementation is way simpler then the previous one.
//! It has no deadlock detection and lacks all features known from the std and parking-lot versions.
//!
//! Uses apple's `os_unfair_lock` for iOS / macOS targets and atomics with `wait` and `wake_one` from the
//! `atomic_wait` crate for all other targets.

#[cfg(any(target_vendor = "apple", test))]
mod simple_mutex_darwin;

#[cfg(any(not(target_vendor = "apple"), test))]
mod simple_mutex_default;

#[cfg(not(target_vendor = "apple"))]
pub use simple_mutex_default::{SMutex, SMutexGuard};

#[cfg(target_vendor = "apple")]
pub use simple_mutex_darwin::{SMutex, SMutexGuard};
