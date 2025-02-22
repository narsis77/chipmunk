//! Provides macros to generate benchmarks with different environments like memory allocators.

// Macros here are used within benchmarks but rust checking isn't able to connect the module
// together yet.
#![allow(unused)]

pub mod mock_producer_multi;
pub mod mock_producer_multi_parallel;
pub mod mock_producer_once;
pub mod mock_producer_once_parallel;
