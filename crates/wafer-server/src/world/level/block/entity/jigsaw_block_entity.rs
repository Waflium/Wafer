use wafer_data::BlockState;
use wafer_protocol::nbt::Nbt;

use crate::world::level::levelgen::structure::templatesystem::structure_template::get_front_facing;
use crate::world::level::storage::tag_value_input;

pub const EMPTY_ID: &str = "minecraft:empty";
pub const DEFAULT_FINAL_STATE: &str = "minecraft:air";
const DEFAULT_PLACEMENT_PRIORITY: i32 = 0;
const DEFAULT_SELECTION_PRIORITY: i32 = 0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum JointType {
    Rollable,
    Aligned,
}

impl JointType {
    pub fn get_serialized_name(self) -> &'static str {
        match self {
            JointType::Rollable => "rollable",
            JointType::Aligned => "aligned",
        }
    }

    pub fn by_name(name: &str) -> Option<JointType> {
        match name {
            "rollable" => Some(JointType::Rollable),
            "aligned" => Some(JointType::Aligned),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct JigsawBlockEntity {
    pub name: String,
    pub target: String,
    pub pool: String,
    pub joint: JointType,
    pub final_state: String,
    pub placement_priority: i32,
    pub selection_priority: i32,
}

impl Default for JigsawBlockEntity {
    fn default() -> JigsawBlockEntity {
        JigsawBlockEntity {
            name: EMPTY_ID.to_owned(),
            target: EMPTY_ID.to_owned(),
            pool: EMPTY_ID.to_owned(),
            joint: JointType::Rollable,
            final_state: DEFAULT_FINAL_STATE.to_owned(),
            placement_priority: DEFAULT_PLACEMENT_PRIORITY,
            selection_priority: DEFAULT_SELECTION_PRIORITY,
        }
    }
}

impl JigsawBlockEntity {
    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("name".into(), Nbt::String(self.name.clone())));
        output.push(("target".into(), Nbt::String(self.target.clone())));
        output.push(("pool".into(), Nbt::String(self.pool.clone())));
        output.push(("final_state".into(), Nbt::String(self.final_state.clone())));
        output.push(("joint".into(), Nbt::String(self.joint.get_serialized_name().into())));
        output.push(("placement_priority".into(), Nbt::Int(self.placement_priority)));
        output.push(("selection_priority".into(), Nbt::Int(self.selection_priority)));
    }

    pub fn load_additional(&mut self, input: &Nbt, state: BlockState) {
        let identifier = |key: &str| tag_value_input::read_or_error(input, key, crate::util::codec::identifier);
        self.name = identifier("name").unwrap_or_else(|| EMPTY_ID.to_owned());
        self.target = identifier("target").unwrap_or_else(|| EMPTY_ID.to_owned());
        self.pool = identifier("pool").unwrap_or_else(|| EMPTY_ID.to_owned());
        self.final_state = tag_value_input::get_string(input, "final_state").unwrap_or(DEFAULT_FINAL_STATE).to_owned();
        self.joint = tag_value_input::read_or_error(input, "joint", |tag| {
            let name = crate::util::codec::string(tag)?;
            JointType::by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))
        })
        .unwrap_or_else(|| get_default_joint_type(state));
        self.placement_priority = super::int_or(input, "placement_priority", DEFAULT_PLACEMENT_PRIORITY);
        self.selection_priority = super::int_or(input, "selection_priority", DEFAULT_SELECTION_PRIORITY);
    }
}

pub fn get_default_joint_type(state: BlockState) -> JointType {
    if get_front_facing(state).axis().is_horizontal() { JointType::Aligned } else { JointType::Rollable }
}
