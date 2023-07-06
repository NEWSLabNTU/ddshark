mod defrag_buf;

pub use defrag_buf::DefragBuf;
use rustdds::{structure::guid::EntityKind, GUID};
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

        use EntityKind as E;

        let entity_kind_str = match entity_id.entity_kind {
            E::UNKNOWN_USER_DEFINED => "U-D",
            E::WRITER_WITH_KEY_USER_DEFINED => "WKD",
            E::WRITER_NO_KEY_USER_DEFINED => "WND",
            E::READER_NO_KEY_USER_DEFINED => "RND",
            E::READER_WITH_KEY_USER_DEFINED => "RKD",
            E::WRITER_GROUP_USER_DEFINED => "WGD",
            E::READER_GROUP_USER_DEFINED => "RGD",
            E::UNKNOWN_BUILT_IN => "U-B",
            E::PARTICIPANT_BUILT_IN => "P-B",
            E::WRITER_WITH_KEY_BUILT_IN => "WKB",
            E::WRITER_NO_KEY_BUILT_IN => "WNB",
            E::READER_NO_KEY_BUILT_IN => "RNB",
            E::READER_WITH_KEY_BUILT_IN => "RKB",
            E::WRITER_GROUP_BUILT_IN => "WGB",
            E::READER_GROUP_BUILT_IN => "RGB",
            _ => unreachable!(),
        };

        write!(
            f,
            "{}|{}|{}",
            hex::encode(prefix.bytes),
            hex::encode(entity_id.entity_key),
            entity_kind_str,
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
