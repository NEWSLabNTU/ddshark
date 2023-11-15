use rustdds::structure::guid::GuidPrefix;
use std::fmt::{self, Display};

/// Extension to [GuidPrefix].
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
        if *self.0 == GuidPrefix::UNKNOWN {
            write!(f, "UNKNOWN")
        } else {
            write!(f, "{}", hex::encode(self.0.bytes))
        }
    }
}
