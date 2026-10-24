use wafer_protocol::nbt::Nbt;

use super::team::{CollisionRule, Visibility};
use super::team_color::TeamColor;
use crate::network::chat::{Component, HoverEvent, Style, component_serialization};
use crate::util::codec::{self, DecodeResult, Record};
use crate::util::java_collections::JavaHashSet;

pub struct PlayerTeam {
    pub(super) name: String,
    pub(super) players: JavaHashSet,
    pub(super) display_name: Component,
    pub(super) player_prefix: Component,
    pub(super) player_suffix: Component,
    pub(super) allow_friendly_fire: bool,
    pub(super) see_friendly_invisibles: bool,
    pub(super) name_tag_visibility: Visibility,
    pub(super) death_message_visibility: Visibility,
    pub(super) color: Option<TeamColor>,
    pub(super) collision_rule: CollisionRule,
    display_name_style: Style,
}

impl PlayerTeam {
    pub fn new(name: &str) -> PlayerTeam {
        PlayerTeam {
            name: name.to_string(),
            players: JavaHashSet::default(),
            display_name: Component::literal(name),
            player_prefix: Component::empty(),
            player_suffix: Component::empty(),
            allow_friendly_fire: true,
            see_friendly_invisibles: true,
            name_tag_visibility: Visibility::Always,
            death_message_visibility: Visibility::Always,
            color: None,
            collision_rule: CollisionRule::Always,
            display_name_style: Style {
                insertion: Some(name.to_string()),
                hover_event: Some(HoverEvent::ShowText(Box::new(Component::literal(name)))),
                ..Style::default()
            },
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn players(&self) -> impl Iterator<Item = &str> {
        self.players.iter()
    }

    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    pub fn display_name(&self) -> &Component {
        &self.display_name
    }

    pub fn player_prefix(&self) -> &Component {
        &self.player_prefix
    }

    pub fn player_suffix(&self) -> &Component {
        &self.player_suffix
    }

    pub fn is_allow_friendly_fire(&self) -> bool {
        self.allow_friendly_fire
    }

    pub fn can_see_friendly_invisibles(&self) -> bool {
        self.see_friendly_invisibles
    }

    pub fn name_tag_visibility(&self) -> Visibility {
        self.name_tag_visibility
    }

    pub fn death_message_visibility(&self) -> Visibility {
        self.death_message_visibility
    }

    pub fn color(&self) -> Option<TeamColor> {
        self.color
    }

    pub fn collision_rule(&self) -> CollisionRule {
        self.collision_rule
    }

    fn apply_color(&self, result: Component) -> Component {
        match self.color {
            Some(color) => result.with_text_color(color.text_color()),
            None => result,
        }
    }

    pub fn formatted_display_name(&self) -> Component {
        let inner = self.display_name.clone().with_style(&self.display_name_style);
        self.apply_color(Component::translatable_with("chat.square_brackets", vec![inner.into()]))
    }

    pub fn formatted_name(&self, member_name: Component) -> Component {
        self.apply_color(Component::empty().append(self.player_prefix.clone()).append(member_name).append(self.player_suffix.clone()))
    }

    pub fn format_name_for_team(team: Option<&PlayerTeam>, name: Component) -> Component {
        match team {
            Some(team) => team.formatted_name(name),
            None => name,
        }
    }

    pub fn is_allied_to(&self, other: Option<&PlayerTeam>) -> bool {
        other.is_some_and(|other| other.name == self.name)
    }

    pub fn pack_options(&self) -> u8 {
        self.allow_friendly_fire as u8 | (self.see_friendly_invisibles as u8) << 1
    }

    pub fn pack(&self) -> Nbt {
        let mut fields = vec![("Name".to_string(), Nbt::String(self.name.clone())), ("DisplayName".into(), self.display_name.to_nbt())];
        if let Some(color) = self.color {
            fields.push(("TeamColor".into(), Nbt::String(color.serialized_name().into())));
        }
        if !self.allow_friendly_fire {
            fields.push(("AllowFriendlyFire".into(), Nbt::Byte(0)));
        }
        if !self.see_friendly_invisibles {
            fields.push(("SeeFriendlyInvisibles".into(), Nbt::Byte(0)));
        }
        if self.player_prefix != Component::empty() {
            fields.push(("MemberNamePrefix".into(), self.player_prefix.to_nbt()));
        }
        if self.player_suffix != Component::empty() {
            fields.push(("MemberNameSuffix".into(), self.player_suffix.to_nbt()));
        }
        if self.name_tag_visibility != Visibility::Always {
            fields.push(("NameTagVisibility".into(), Nbt::String(self.name_tag_visibility.name().into())));
        }
        if self.death_message_visibility != Visibility::Always {
            fields.push(("DeathMessageVisibility".into(), Nbt::String(self.death_message_visibility.name().into())));
        }
        if self.collision_rule != CollisionRule::Always {
            fields.push(("CollisionRule".into(), Nbt::String(self.collision_rule.name().into())));
        }
        if !self.players.is_empty() {
            fields.push(("Players".into(), Nbt::List(self.players.iter().map(|p| Nbt::String(p.into())).collect())));
        }
        Nbt::Compound(fields)
    }
}

pub struct PackedTeam {
    pub name: String,
    pub display_name: Option<Component>,
    pub color: Option<TeamColor>,
    pub allow_friendly_fire: bool,
    pub see_friendly_invisibles: bool,
    pub member_name_prefix: Component,
    pub member_name_suffix: Component,
    pub name_tag_visibility: Visibility,
    pub death_message_visibility: Visibility,
    pub collision_rule: CollisionRule,
    pub players: Vec<String>,
}

impl PackedTeam {
    pub fn decode(tag: &Nbt) -> DecodeResult<PackedTeam> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let name = record.required(tag, "Name", |name| codec::string(name).map(str::to_string));
        let display_name = record.optional(tag, "DisplayName", component_serialization::decode_partial);
        let color = record.optional(tag, "TeamColor", |color| codec::enum_by_name(color, TeamColor::by_name));
        let allow_friendly_fire = record.with_default(tag, "AllowFriendlyFire", true, codec::bool_value);
        let see_friendly_invisibles = record.with_default(tag, "SeeFriendlyInvisibles", true, codec::bool_value);
        let member_name_prefix = record.with_default(tag, "MemberNamePrefix", Component::empty(), component_serialization::decode_partial);
        let member_name_suffix = record.with_default(tag, "MemberNameSuffix", Component::empty(), component_serialization::decode_partial);
        let name_tag_visibility = record.with_default(tag, "NameTagVisibility", Visibility::Always, |visibility| codec::enum_by_name(visibility, Visibility::by_name));
        let death_message_visibility = record.with_default(tag, "DeathMessageVisibility", Visibility::Always, |visibility| codec::enum_by_name(visibility, Visibility::by_name));
        let collision_rule = record.with_default(tag, "CollisionRule", CollisionRule::Always, |rule| codec::enum_by_name(rule, CollisionRule::by_name));
        let players = record.with_default(tag, "Players", Vec::new(), |players| codec::list_partial(players, |player| codec::string(player).map(str::to_string).into()));
        record.build(|| {
            Some(PackedTeam {
                name: name?,
                display_name,
                color,
                allow_friendly_fire,
                see_friendly_invisibles,
                member_name_prefix,
                member_name_suffix,
                name_tag_visibility,
                death_message_visibility,
                collision_rule,
                players,
            })
        })
    }
}
