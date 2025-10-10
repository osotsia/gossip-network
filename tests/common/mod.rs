//! tests/common/mod.rs
//!
//! Public facade for the test harness module. This makes test utilities
//! easily accessible to all integration test files.

// The #[allow(dead_code)] attribute is applied here because this is a library
// of test utilities. Not all functions will be used in every test file,
// which would otherwise trigger compiler warnings.
#![allow(dead_code)]

pub mod harness;