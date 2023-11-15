use crate::utils::EntityKindExt;
use rustdds::structure::guid::EntityId;
use std::fmt::{self, Display};

/// Extension to [EntityId].
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

                return write!(f, "{}|{}", hex::encode(entity_key), entity_kind.display());
            }
        };

        write!(f, "{}", name)
    }
}
