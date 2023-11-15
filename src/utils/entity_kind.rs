use rustdds::structure::guid::EntityKind;
use std::fmt::{self, Display};

/// Extension to [EntityKind].
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
