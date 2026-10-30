use super::upgrade_progress::{Status, UpgradeProgress};
use crate::network::chat::Component;
use crate::util::datafix::data_fix_types::DataFixTypes;

const FAILED: &str = "optimizeWorld.stage.failed";
const COUNTING: &str = "optimizeWorld.stage.counting";
const UPGRADING: &str = "optimizeWorld.stage.upgrading";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StatusMessage(&'static str);

impl StatusMessage {
    pub fn component(self) -> Component {
        Component::translatable(self.0)
    }
}

struct Messages {
    upgrading: StatusMessage,
    finished: StatusMessage,
}

impl Messages {
    fn for_status(&self, status: Status) -> StatusMessage {
        match status {
            Status::Upgrading => self.upgrading,
            Status::Finished => self.finished,
            _ => panic!("java.lang.IllegalStateException: Invalid Status received: {status:?}"),
        }
    }
}

const CHUNK: Messages = Messages { upgrading: StatusMessage("optimizeWorld.stage.upgrading.chunks"), finished: StatusMessage("optimizeWorld.stage.finished.chunks") };
const ENTITY_CHUNK: Messages = Messages { upgrading: StatusMessage("optimizeWorld.stage.upgrading.entities"), finished: StatusMessage("optimizeWorld.stage.finished.entities") };
const POI_CHUNK: Messages = Messages { upgrading: StatusMessage("optimizeWorld.stage.upgrading.poi"), finished: StatusMessage("optimizeWorld.stage.finished.poi") };

#[derive(Default)]
pub struct UpgradeStatusTranslator;

impl UpgradeStatusTranslator {
    pub fn translate(&self, upgrade_progress: &UpgradeProgress) -> StatusMessage {
        let status = upgrade_progress.get_status();
        if status == Status::Failed {
            return StatusMessage(FAILED);
        }
        if status == Status::Counting {
            return StatusMessage(COUNTING);
        }
        let Some(data_fix_type) = upgrade_progress.get_data_fix_type() else { return StatusMessage(COUNTING) };
        let type_messages = match data_fix_type {
            DataFixTypes::Chunk => Some(CHUNK),
            DataFixTypes::EntityChunk => Some(ENTITY_CHUNK),
            DataFixTypes::PoiChunk => Some(POI_CHUNK),
            _ => None,
        };
        match type_messages {
            Some(type_messages) => type_messages.for_status(status),
            None => StatusMessage(UPGRADING),
        }
    }
}
