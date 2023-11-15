use rustdds::structure::locator::Locator;
use std::fmt::{self, Display};

/// Extension to [Locator].
pub trait LocatorExt {
    fn display(&self) -> LocatorDisplay<'_>;
}

impl LocatorExt for Locator {
    fn display(&self) -> LocatorDisplay<'_> {
        LocatorDisplay(self)
    }
}

pub struct LocatorDisplay<'a>(&'a Locator);

impl<'a> Display for LocatorDisplay<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Locator::Invalid => write!(f, "invalid/"),
            Locator::Reserved => write!(f, "reserved/"),
            Locator::UdpV4(addr) => write!(f, "udp/{addr}"),
            Locator::UdpV6(addr) => write!(f, "udp/{addr}"),
            Locator::Other {
                kind,
                port,
                address,
            } => write!(
                f,
                "other/kind={kind:04x},port={port:04x},addr={:032}",
                hex::encode(address)
            ),
        }
    }
}
