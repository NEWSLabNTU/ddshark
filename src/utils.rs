use rustdds::GUID;
use std::{
    fmt::{self, Display},
    mem::ManuallyDrop,
};

pub trait VecExt<T> {
    fn into_raw_parts_(self) -> (*mut T, usize, usize);
}

impl<T> VecExt<T> for Vec<T> {
    fn into_raw_parts_(self) -> (*mut T, usize, usize) {
        let mut me = ManuallyDrop::new(self);
        (me.as_mut_ptr(), me.len(), me.capacity())
    }
}

pub trait GUIDExt {
    fn display(&self) -> GUIDDisplay<'_>;
}

pub struct GUIDDisplay<'a>(&'a GUID);

impl<'a> Display for GUIDDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let GUID { prefix, entity_id } = self.0;
        write!(
            f,
            "{}{}{:02x}",
            hex::encode(prefix.bytes),
            hex::encode(entity_id.entity_key),
            entity_id.entity_kind.0,
        )
    }
}

impl GUIDExt for GUID {
    fn display(&self) -> GUIDDisplay<'_> {
        GUIDDisplay(self)
    }
}

pub fn num_base10_digits_usize(val: usize) -> u32 {
    val.checked_ilog10().unwrap_or(0) + 1
}

pub fn num_base10_digits_i64(val: i64) -> u32 {
    val.checked_ilog10().unwrap_or(0) + 1
}
