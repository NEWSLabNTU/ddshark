mod defrag_buf;

pub use defrag_buf::DefragBuf;
use rustdds::{
    structure::guid::{EntityId, EntityKind, GuidPrefix},
    GUID,
};
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

impl GUIDExt for GUID {
    fn display(&self) -> GUIDDisplay<'_> {
        GUIDDisplay(self)
    }
}

pub struct GUIDDisplay<'a>(&'a GUID);

impl<'a> Display for GUIDDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let GUID { prefix, entity_id } = self.0;

        write!(f, "{}|{}", hex::encode(prefix.bytes), entity_id.display())
    }
}

pub trait GuidPrefixExt {
    fn display(&self) -> GuidPrefixDisplay<'_>;
}

impl GuidPrefixExt for GuidPrefix {
    fn display(&self) -> GuidPrefixDisplay<'_> {
        GuidPrefixDisplay(self)
    }
}

pub struct GuidPrefixDisplay<'a>(&'a GuidPrefix);

impl<'a> Display for GuidPrefixDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", hex::encode(self.0.bytes),)
    }
}

pub trait EntityIdExt {
    fn display(&self) -> EntityIdDisplay<'_>;
}

impl EntityIdExt for EntityId {
    fn display(&self) -> EntityIdDisplay<'_> {
        EntityIdDisplay(self)
    }
}

pub struct EntityIdDisplay<'a>(&'a EntityId);

impl<'a> Display for EntityIdDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match *self.0 {
            EntityId::SEDP_BUILTIN_TOPIC_WRITER => "SEDP_BUILTIN_TOPIC_WRITER",
            EntityId::SEDP_BUILTIN_TOPIC_READER => "SEDP_BUILTIN_TOPIC_READER",
            EntityId::SEDP_BUILTIN_PUBLICATIONS_WRITER => "SEDP_BUILTIN_PUBLICATIONS_WRITER",
            EntityId::SEDP_BUILTIN_PUBLICATIONS_READER => "SEDP_BUILTIN_PUBLICATIONS_READER",
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_WRITER => "SEDP_BUILTIN_SUBSCRIPTIONS_WRITER",
            EntityId::SEDP_BUILTIN_SUBSCRIPTIONS_READER => "SEDP_BUILTIN_SUBSCRIPTIONS_READER",
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER => "SPDP_BUILTIN_PARTICIPANT_WRITER",
            EntityId::SPDP_BUILTIN_PARTICIPANT_READER => "SPDP_BUILTIN_PARTICIPANT_READER",
            EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER => {
                "P2P_BUILTIN_PARTICIPANT_MESSAGE_WRITER"
            }
            EntityId::P2P_BUILTIN_PARTICIPANT_MESSAGE_READER => {
                "P2P_BUILTIN_PARTICIPANT_MESSAGE_READER"
            }
            _ => {
                let EntityId {
                    entity_key,
                    entity_kind,
                } = self.0;

                return write!(f, "{}|{}", hex::encode(entity_key), entity_kind.display(),);
            }
        };

        write!(f, "{}", name)
    }
}

pub trait EntityKindExt {
    fn display(&self) -> EntityKindDisplay<'_>;
}

impl EntityKindExt for EntityKind {
    fn display(&self) -> EntityKindDisplay<'_> {
        EntityKindDisplay(self)
    }
}

pub struct EntityKindDisplay<'a>(&'a EntityKind);

impl<'a> Display for EntityKindDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use EntityKind as E;

        let text = match *self.0 {
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
        write!(f, "{}", text)
    }
}

pub fn num_base10_digits_usize(val: usize) -> u32 {
    val.checked_ilog10().unwrap_or(0) + 1
}

pub fn num_base10_digits_i64(val: i64) -> u32 {
    val.checked_ilog10().unwrap_or(0) + 1
}
