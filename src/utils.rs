//! Utility types and functions.

mod entity_id;
mod entity_kind;
mod guid;
mod guid_prefix;
mod locator;
mod timed_stat;
mod vec;

pub use entity_id::*;
pub use entity_kind::*;
pub use guid::*;
pub use guid_prefix::*;
pub use locator::*;
pub use timed_stat::*;

// pub fn num_base10_digits_usize(val: usize) -> u32 {
//     val.checked_ilog10().unwrap_or(0) + 1
// }

// pub fn num_base10_digits_i64(val: i64) -> u32 {
//     val.checked_ilog10().unwrap_or(0) + 1
// }
