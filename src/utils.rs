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

#[cfg(test)]
mod tests {
    use super::*;
    use rustdds::{
        structure::{
            guid::{EntityId, EntityKind, GuidPrefix},
            locator::Locator,
        },
        GUID,
    };
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[test]
    fn entity_kind_display() {
        assert_eq!(
            EntityKind::WRITER_NO_KEY_USER_DEFINED.display().to_string(),
            "WND"
        );
        assert_eq!(
            EntityKind::PARTICIPANT_BUILT_IN.display().to_string(),
            "P-B"
        );
    }

    #[test]
    fn guid_prefix_unknown_displays_unknown() {
        assert_eq!(GuidPrefix::UNKNOWN.display().to_string(), "UNKNOWN");
    }

    #[test]
    fn guid_with_unknown_prefix_displays_unknown() {
        let guid = GUID::new(
            GuidPrefix::UNKNOWN,
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER,
        );
        assert_eq!(guid.display().to_string(), "UNKNOWN");
    }

    #[test]
    fn entity_id_builtin_name() {
        assert_eq!(
            EntityId::SPDP_BUILTIN_PARTICIPANT_WRITER
                .display()
                .to_string(),
            "SPDP_BUILTIN_PARTICIPANT_WRITER"
        );
    }

    #[test]
    fn locator_display() {
        assert_eq!(Locator::Invalid.display().to_string(), "invalid/");
        let loc = Locator::UdpV4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 7400));
        assert_eq!(loc.display().to_string(), "udp/127.0.0.1:7400");
    }
}
