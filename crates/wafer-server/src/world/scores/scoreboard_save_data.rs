use std::io;
use std::path::PathBuf;

use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::display_slot::DisplaySlot;
use super::objective::PackedObjective;
use super::player_team::PackedTeam;
use super::score::Score;
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::level::storage::{read_saved_data, write_dat};

pub struct PackedScore {
    pub owner: String,
    pub objective: String,
    pub score: Score,
}

impl PackedScore {
    fn decode(tag: &Nbt) -> DecodeResult<PackedScore> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let owner = record.required(tag, "Name", |owner| codec::string(owner).map(str::to_string));
        let objective = record.required(tag, "Objective", |objective| codec::string(objective).map(str::to_string));
        let score = record.absorb(Score::decode_packed(tag));
        record.build(|| Some(PackedScore { owner: owner?, objective: objective?, score: score? }))
    }
}

#[derive(Default)]
pub struct PackedScoreboardSaveData {
    pub objectives: Vec<PackedObjective>,
    pub scores: Vec<PackedScore>,
    pub display_slots: Vec<(DisplaySlot, String)>,
    pub teams: Vec<PackedTeam>,
}

impl PackedScoreboardSaveData {
    pub fn decode(data: &Nbt) -> DecodeResult<PackedScoreboardSaveData> {
        if let Err(error) = codec::map(data) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let objectives = record.with_default(data, "Objectives", Vec::new(), |objectives| codec::list_partial(objectives, PackedObjective::decode));
        let scores = record.with_default(data, "PlayerScores", Vec::new(), |scores| codec::list_partial(scores, PackedScore::decode));
        let display_slots = record.with_default(data, "DisplaySlots", Vec::new(), |slots| {
            codec::base_map_partial(slots, |slot| DisplaySlot::by_name(slot).ok_or_else(|| format!("Unknown element name:{slot}")), |name| codec::string(name).map(str::to_string).into())
        });
        let teams = record.with_default(data, "Teams", Vec::new(), |teams| codec::list_partial(teams, PackedTeam::decode));
        record.build(|| Some(PackedScoreboardSaveData { objectives, scores, display_slots, teams }))
    }
}

pub struct ScoreboardSaveData {
    dir: Option<PathBuf>,
    packed: PackedScoreboardSaveData,
    data: Nbt,
    dirty: bool,
}

impl ScoreboardSaveData {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("scoreboard.dat")
    }

    pub fn load(dir: Option<PathBuf>) -> ScoreboardSaveData {
        let mut raw = None;
        let packed = dir.as_deref().and_then(|dir| {
            read_saved_data(&ScoreboardSaveData::file(dir), "Server thread", "scoreboard", DataFixTypes::SavedDataScoreboard, |data| {
                raw = Some(data.clone());
                PackedScoreboardSaveData::decode(data)
            })
        });
        let dirty = packed.is_none();
        ScoreboardSaveData { dir, packed: packed.unwrap_or_default(), data: raw.filter(|_| !dirty).unwrap_or(Nbt::Compound(Vec::new())), dirty }
    }

    pub fn take_packed(&mut self) -> PackedScoreboardSaveData {
        std::mem::take(&mut self.packed)
    }

    pub fn set_data(&mut self, data: Nbt) {
        if data != self.data {
            self.data = data;
            self.dirty = true;
        }
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let file = Nbt::Compound(vec![("data".into(), self.data.clone()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&ScoreboardSaveData::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_errors_match_vanilla() {
        let cases: [(&str, Option<&str>, [usize; 4]); 17] = [
            ("{}", None, [0, 0, 0, 0]),
            ("{Objectives:5}", Some("Not a list: 5"), [0, 0, 0, 0]),
            ("{Objectives:[5]}", Some("Not a map: 5"), [0, 0, 0, 0]),
            ("{Objectives:[{}]}", Some("No key DisplayName in MapLike[{}]; No key Name in MapLike[{}]"), [0, 0, 0, 0]),
            (r#"{Objectives:[{Name:"a",DisplayName:"A"}]}"#, None, [1, 0, 0, 0]),
            (
                r#"{Objectives:[{Name:"a",DisplayName:"A",CriteriaName:"nope",RenderType:"x",display_auto_update:"q"}]}"#,
                Some("Not a number; Unknown element name:x; No scoreboard criteria with name: nope"),
                [1, 0, 0, 0],
            ),
            (
                r#"{Objectives:[{Name:5,DisplayName:"A",format:{type:"minecraft:nope"}}]}"#,
                Some("Unknown registry key in ResourceKey[minecraft:root / minecraft:number_format_type]: minecraft:nope; Not a string"),
                [0, 0, 0, 0],
            ),
            (r#"{Objectives:[{Name:"a",DisplayName:"A"},{Name:"b"}]}"#, Some(r#"No key DisplayName in MapLike[{Name:"b"}]"#), [1, 0, 0, 0]),
            (r#"{PlayerScores:[{Name:"x",Objective:"a"}]}"#, None, [0, 1, 0, 0]),
            (
                r#"{PlayerScores:[{Name:"x",Objective:"a",Score:"s",Locked:"l"},{Name:"y"}]}"#,
                Some(r#"No key Objective in MapLike[{Name:"y"}]; Not a number; Not a number"#),
                [0, 1, 0, 0],
            ),
            (r#"{PlayerScores:[{Name:"x",Objective:"a",Score:3,Locked:1b,display:"hi",format:{type:"minecraft:blank"}}]}"#, None, [0, 1, 0, 0]),
            ("{DisplaySlots:5}", Some("Not a map: 5"), [0, 0, 0, 0]),
            (r#"{DisplaySlots:{"sidebar":"a","bad":"b","list":5}}"#, Some(r#"Not a string; Unknown element name:bad missed input: {bad:"b",list:5}"#), [0, 0, 1, 0]),
            ("{Teams:[{}]}", Some("No key Name in MapLike[{}]"), [0, 0, 0, 0]),
            (
                r#"{Teams:[{Name:"t",TeamColor:"nope",AllowFriendlyFire:"x",NameTagVisibility:"x",CollisionRule:5,Players:["a",5,"b"],MemberNamePrefix:5}]}"#,
                Some(
                    "Not a string; Not a string; Unknown element name:x; Failed to parse either. First: Failed to parse either. First: Not a string; Second: Not a list: 5; Second: Not a map: 5; Not a number; Unknown element name:nope",
                ),
                [0, 0, 0, 1],
            ),
            (r#"{Teams:[{Name:"t",TeamColor:"red",NameTagVisibility:"never",CollisionRule:"pushOtherTeams",Players:["a","b"],DisplayName:"D"},5]}"#, Some("Not a map: 5"), [0, 0, 0, 1]),
            ("{Objectives:5,PlayerScores:6,DisplaySlots:7,Teams:8}", Some("Not a list: 8; Not a map: 7; Not a list: 6; Not a list: 5"), [0, 0, 0, 0]),
        ];
        for (input, error, counts) in cases {
            let tag = crate::nbt::parse_fully(input).ok().unwrap();
            let result = codec::with_nbt_ops(|| PackedScoreboardSaveData::decode(&tag));
            assert_eq!(result.error.as_deref(), error, "{input}");
            let packed = result.value.unwrap();
            assert_eq!([packed.objectives.len(), packed.scores.len(), packed.display_slots.len(), packed.teams.len()], counts, "{input}");
        }
    }
}
