use rustdds::{structure::guid::GuidPrefix, GUID};
use std::fmt::{self, Display};

use crate::utils::EntityIdExt;

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
        if self.0.prefix == GuidPrefix::UNKNOWN {
            write!(f, "UNKNOWN")
        } else {
            let GUID { prefix, entity_id } = self.0;
            write!(f, "{}|{}", hex::encode(prefix.bytes), entity_id.display())
        }
    }
}
