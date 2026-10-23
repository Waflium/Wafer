use wafer_data::{Block, BlockState, registry};
use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::{read_identifier, registry_tag};
use crate::commands::brigadier::{
    CommandSyntaxException, Dynamic2CommandExceptionType, Dynamic3CommandExceptionType, DynamicCommandExceptionType, SimpleCommandExceptionType, StringReader,
    tr,
};
use crate::commands::shared_suggestion_provider;
use crate::commands::suggestion::{Suggestions, SuggestionsBuilder};
use crate::nbt;

pub static ERROR_NO_TAGS_ALLOWED: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.block.tag.disallowed");
pub static ERROR_UNKNOWN_BLOCK: DynamicCommandExceptionType = DynamicCommandExceptionType(|id| tr("argument.block.id.invalid", vec![id]));
pub static ERROR_UNKNOWN_PROPERTY: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.unknown", vec![block, property]));
pub static ERROR_DUPLICATE_PROPERTY: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.duplicate", vec![property, block]));
pub static ERROR_INVALID_VALUE: Dynamic3CommandExceptionType =
    Dynamic3CommandExceptionType(|block, property, value| tr("argument.block.property.invalid", vec![block, value, property]));
pub static ERROR_EXPECTED_VALUE: Dynamic2CommandExceptionType =
    Dynamic2CommandExceptionType(|block, property| tr("argument.block.property.novalue", vec![property, block]));
pub static ERROR_EXPECTED_END_OF_PROPERTIES: SimpleCommandExceptionType = SimpleCommandExceptionType("argument.block.property.unclosed");
pub static ERROR_UNKNOWN_TAG: DynamicCommandExceptionType = DynamicCommandExceptionType(|tag| tr("arguments.block.tag.unknown", vec![tag]));

pub struct BlockResult {
    pub state: BlockState,
    pub properties: Vec<&'static str>,
    pub nbt: Option<Nbt>,
}

pub struct TagResult {
    pub blocks: &'static [i32],
    pub vague_properties: Vec<(String, String)>,
    pub nbt: Option<Nbt>,
}

pub enum ParseResult {
    Block(BlockResult),
    Tag(TagResult),
}

enum Suggest {
    Nothing,
    BlockIdOrTag,
    Item,
    Tag,
    OpenVaguePropertiesOrNbt,
    OpenPropertiesOrNbt,
    OpenNbt,
    PropertyNameOrEnd,
    VaguePropertyNameOrEnd,
    PropertyName,
    VaguePropertyName,
    Equals,
    NextPropertyOrEnd,
    PropertyValue(&'static [&'static str]),
    VaguePropertyValue(String),
}

struct BlockStateParser<'r, 'a> {
    reader: &'r mut StringReader<'a>,
    for_testing: bool,
    allow_nbt: bool,
    properties: Vec<&'static str>,
    vague_properties: Vec<(String, String)>,
    id: String,
    state: Option<BlockState>,
    tag: Option<&'static [i32]>,
    nbt: Option<Nbt>,
    suggestions: Suggest,
}

pub fn fill_suggestions(builder: &SuggestionsBuilder, for_testing: bool, allow_nbt: bool) -> Suggestions {
    let input = builder.input().to_vec();
    let mut reader = StringReader::new(&input);
    reader.cursor = builder.start();
    let mut parser = BlockStateParser::new(&mut reader, for_testing, allow_nbt);
    let _ = parser.parse();
    let offset = parser.reader.cursor;
    parser.apply_suggestions(builder.create_offset(offset))
}

pub fn parse_for_block(reader: &mut StringReader, allow_nbt: bool) -> Result<BlockResult, CommandSyntaxException> {
    let cursor = reader.cursor;
    let mut parser = BlockStateParser::new(reader, false, allow_nbt);
    match parser.parse() {
        Ok(()) => Ok(BlockResult { state: parser.state.expect("a parsed block"), properties: parser.properties, nbt: parser.nbt }),
        Err(e) => {
            reader.cursor = cursor;
            Err(e)
        }
    }
}

pub fn parse_for_testing(reader: &mut StringReader, allow_nbt: bool) -> Result<ParseResult, CommandSyntaxException> {
    let cursor = reader.cursor;
    let mut parser = BlockStateParser::new(reader, true, allow_nbt);
    match parser.parse() {
        Ok(()) => Ok(match parser.tag {
            Some(blocks) => ParseResult::Tag(TagResult { blocks, vague_properties: parser.vague_properties, nbt: parser.nbt }),
            None => ParseResult::Block(BlockResult { state: parser.state.expect("a parsed block"), properties: parser.properties, nbt: parser.nbt }),
        }),
        Err(e) => {
            reader.cursor = cursor;
            Err(e)
        }
    }
}

pub fn property_value(values: &'static [&'static str], raw: &str) -> Option<&'static str> {
    let integer = values.iter().all(|v| v.parse::<i32>().is_ok());
    if integer {
        let value = java_parse_int(raw)?;
        values.iter().copied().find(|v| v.parse::<i32>() == Ok(value))
    } else {
        values.iter().copied().find(|v| *v == raw)
    }
}

fn add_suggestions(builder: &mut SuggestionsBuilder, values: &[&str]) {
    if values.iter().all(|v| v.parse::<i32>().is_ok()) {
        for value in values {
            if let Ok(value) = value.parse() {
                builder.suggest_int(value);
            }
        }
    } else {
        for value in values {
            builder.suggest(*value);
        }
    }
}

fn java_parse_int(s: &str) -> Option<i32> {
    let digits = s.strip_prefix(['-', '+']).unwrap_or(s);
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().try_fold(0i64, |v, c| v.checked_mul(10)?.checked_add((c - b'0') as i64).filter(|&v| v <= 1 << 31))?;
    i32::try_from(if s.starts_with('-') { -magnitude } else { magnitude }).ok()
}

impl<'r, 'a> BlockStateParser<'r, 'a> {
    fn new(reader: &'r mut StringReader<'a>, for_testing: bool, allow_nbt: bool) -> Self {
        BlockStateParser {
            reader,
            for_testing,
            allow_nbt,
            properties: Vec::new(),
            vague_properties: Vec::new(),
            id: "minecraft:".into(),
            state: None,
            tag: None,
            nbt: None,
            suggestions: Suggest::Nothing,
        }
    }

    fn parse(&mut self) -> Result<(), CommandSyntaxException> {
        self.suggestions = if self.for_testing { Suggest::BlockIdOrTag } else { Suggest::Item };
        if self.reader.at('#') {
            self.read_tag()?;
            self.suggestions = Suggest::OpenVaguePropertiesOrNbt;
            if self.reader.at('[') {
                self.read_vague_properties()?;
                self.suggestions = Suggest::OpenNbt;
            }
        } else {
            self.read_block()?;
            self.suggestions = Suggest::OpenPropertiesOrNbt;
            if self.reader.at('[') {
                self.read_properties()?;
                self.suggestions = Suggest::OpenNbt;
            }
        }
        if self.allow_nbt && self.reader.at('{') {
            self.suggestions = Suggest::Nothing;
            self.nbt = Some(nbt::parse_compound_as_argument(self.reader)?);
        }
        Ok(())
    }

    fn apply_suggestions(&self, mut builder: SuggestionsBuilder) -> Suggestions {
        match &self.suggestions {
            Suggest::Nothing => {}
            Suggest::BlockIdOrTag => {
                self.suggest_tag(&mut builder);
                self.suggest_item(&mut builder);
            }
            Suggest::Item => self.suggest_item(&mut builder),
            Suggest::Tag => self.suggest_tag(&mut builder),
            Suggest::OpenVaguePropertiesOrNbt => self.suggest_open_vague_properties_or_nbt(&mut builder),
            Suggest::OpenPropertiesOrNbt => self.suggest_open_properties_or_nbt(&mut builder),
            Suggest::OpenNbt => self.suggest_open_nbt(&mut builder),
            Suggest::PropertyNameOrEnd => {
                if builder.remaining().is_empty() {
                    builder.suggest("]");
                }
                self.suggest_property_name(&mut builder);
            }
            Suggest::VaguePropertyNameOrEnd => {
                if builder.remaining().is_empty() {
                    builder.suggest("]");
                }
                self.suggest_vague_property_name(&mut builder);
            }
            Suggest::PropertyName => self.suggest_property_name(&mut builder),
            Suggest::VaguePropertyName => self.suggest_vague_property_name(&mut builder),
            Suggest::Equals => {
                if builder.remaining().is_empty() {
                    builder.suggest("=");
                }
            }
            Suggest::NextPropertyOrEnd => {
                if builder.remaining().is_empty() {
                    builder.suggest("]");
                    if self.properties.len() < self.state.expect("a parsed block").block().info().properties.len() {
                        builder.suggest(",");
                    }
                }
            }
            Suggest::PropertyValue(values) => add_suggestions(&mut builder, values),
            Suggest::VaguePropertyValue(key) => self.suggest_vague_property_value(&mut builder, key),
        }
        builder.build()
    }

    fn suggest_property_name(&self, builder: &mut SuggestionsBuilder) {
        let prefix = builder.remaining().to_lowercase();
        for (name, _) in self.state.expect("a parsed block").properties() {
            if !self.properties.contains(&name) && name.starts_with(&prefix) {
                builder.suggest(format!("{name}="));
            }
        }
    }

    fn suggest_vague_property_name(&self, builder: &mut SuggestionsBuilder) {
        let prefix = builder.remaining().to_lowercase();
        if let Some(tag) = self.tag {
            for &block in tag {
                for (name, _) in Block(block as u16).default_state().properties() {
                    if !self.vague_properties.iter().any(|(key, _)| key == name) && name.starts_with(&prefix) {
                        builder.suggest(format!("{name}="));
                    }
                }
            }
        }
    }

    fn suggest_open_nbt(&self, builder: &mut SuggestionsBuilder) {
        if builder.remaining().is_empty() && self.has_block_entity() {
            builder.suggest("{");
        }
    }

    fn has_block_entity(&self) -> bool {
        if let Some(state) = self.state {
            return state.has_block_entity();
        }
        self.tag.is_some_and(|tag| tag.iter().any(|&block| Block(block as u16).default_state().has_block_entity()))
    }

    fn suggest_vague_property_value(&self, builder: &mut SuggestionsBuilder, key: &str) {
        let mut has_more_properties = false;
        if let Some(tag) = self.tag {
            for &block in tag {
                let block = Block(block as u16);
                if let Some((_, values)) = block.property_by_name(key) {
                    add_suggestions(builder, values);
                }
                if !has_more_properties {
                    has_more_properties = block.default_state().properties().any(|(name, _)| !self.vague_properties.iter().any(|(vague, _)| vague == name));
                }
            }
        }
        if has_more_properties {
            builder.suggest(",");
        }
        builder.suggest("]");
    }

    fn suggest_open_vague_properties_or_nbt(&self, builder: &mut SuggestionsBuilder) {
        if builder.remaining().is_empty()
            && let Some(tag) = self.tag
        {
            let mut has_properties = false;
            let mut has_entity = false;
            for &block in tag {
                let default_state = Block(block as u16).default_state();
                has_properties |= default_state.properties().next().is_some();
                has_entity |= default_state.has_block_entity();
                if has_properties && has_entity {
                    break;
                }
            }
            if has_properties {
                builder.suggest("[");
            }
            if has_entity {
                builder.suggest("{");
            }
        }
    }

    fn suggest_open_properties_or_nbt(&self, builder: &mut SuggestionsBuilder) {
        if builder.remaining().is_empty() {
            let state = self.state.expect("a parsed block");
            if !state.block().info().properties.is_empty() {
                builder.suggest("[");
            }
            if state.has_block_entity() {
                builder.suggest("{");
            }
        }
    }

    fn suggest_tag(&self, builder: &mut SuggestionsBuilder) {
        let tags = wafer_data::tags().iter().find(|r| r.registry == "minecraft:block").map_or(&[][..], |r| r.tags);
        shared_suggestion_provider::suggest_resource_with_prefix(tags.iter().map(|t| t.0), builder, "#");
    }

    fn suggest_item(&self, builder: &mut SuggestionsBuilder) {
        shared_suggestion_provider::suggest_resource(registry::BLOCK.iter(), builder);
    }

    fn read_block(&mut self) -> Result<(), CommandSyntaxException> {
        let start = self.reader.cursor;
        self.id = read_identifier(self.reader)?;
        let Some(block) = Block::by_name(&self.id) else {
            self.reader.cursor = start;
            return Err(ERROR_UNKNOWN_BLOCK.create_with_context(self.reader, self.id.as_str()));
        };
        self.state = Some(block.default_state());
        Ok(())
    }

    fn read_tag(&mut self) -> Result<(), CommandSyntaxException> {
        if !self.for_testing {
            return Err(ERROR_NO_TAGS_ALLOWED.create_with_context(self.reader));
        }
        let start = self.reader.cursor;
        self.reader.expect('#')?;
        self.suggestions = Suggest::Tag;
        let id = read_identifier(self.reader)?;
        match registry_tag("minecraft:block", &id) {
            Some(tag) => self.tag = Some(tag),
            None => {
                self.reader.cursor = start;
                return Err(ERROR_UNKNOWN_TAG.create_with_context(self.reader, id));
            }
        }
        Ok(())
    }

    fn read_properties(&mut self) -> Result<(), CommandSyntaxException> {
        self.reader.skip();
        self.suggestions = Suggest::PropertyNameOrEnd;
        self.reader.skip_whitespace();
        let block = self.state.expect("a parsed block").block();
        while self.reader.can_read() && !self.reader.at(']') {
            self.reader.skip_whitespace();
            let key_start = self.reader.cursor;
            let key = self.reader.read_string()?;
            let Some((property, values)) = block.property_by_name(&key) else {
                self.reader.cursor = key_start;
                return Err(ERROR_UNKNOWN_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            };
            if self.properties.contains(&property) {
                self.reader.cursor = key_start;
                return Err(ERROR_DUPLICATE_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip_whitespace();
            self.suggestions = Suggest::Equals;
            if !self.reader.at('=') {
                return Err(ERROR_EXPECTED_VALUE.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip();
            self.reader.skip_whitespace();
            self.suggestions = Suggest::PropertyValue(values);
            let start = self.reader.cursor;
            let raw = self.reader.read_string()?;
            self.set_value(property, values, &raw, start)?;
            self.suggestions = Suggest::NextPropertyOrEnd;
            self.reader.skip_whitespace();
            if self.reader.can_read() {
                if !self.reader.at(',') {
                    if !self.reader.at(']') {
                        return Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader));
                    }
                    break;
                }
                self.reader.skip();
                self.suggestions = Suggest::PropertyName;
            }
        }
        if self.reader.can_read() {
            self.reader.skip();
            Ok(())
        } else {
            Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader))
        }
    }

    fn read_vague_properties(&mut self) -> Result<(), CommandSyntaxException> {
        self.reader.skip();
        self.suggestions = Suggest::VaguePropertyNameOrEnd;
        let mut value_start = None;
        self.reader.skip_whitespace();
        while self.reader.can_read() && !self.reader.at(']') {
            self.reader.skip_whitespace();
            let key_start = self.reader.cursor;
            let key = self.reader.read_string()?;
            if self.vague_properties.iter().any(|(k, _)| *k == key) {
                self.reader.cursor = key_start;
                return Err(ERROR_DUPLICATE_PROPERTY.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip_whitespace();
            if !self.reader.at('=') {
                self.reader.cursor = key_start;
                return Err(ERROR_EXPECTED_VALUE.create_with_context(self.reader, self.id.as_str(), key));
            }
            self.reader.skip();
            self.reader.skip_whitespace();
            self.suggestions = Suggest::VaguePropertyValue(key.clone());
            value_start = Some(self.reader.cursor);
            let value = self.reader.read_string()?;
            self.vague_properties.push((key, value));
            self.reader.skip_whitespace();
            if self.reader.can_read() {
                value_start = None;
                if !self.reader.at(',') {
                    if !self.reader.at(']') {
                        return Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader));
                    }
                    break;
                }
                self.reader.skip();
                self.suggestions = Suggest::VaguePropertyName;
            }
        }
        if self.reader.can_read() {
            self.reader.skip();
            return Ok(());
        }
        if let Some(start) = value_start {
            self.reader.cursor = start;
        }
        Err(ERROR_EXPECTED_END_OF_PROPERTIES.create_with_context(self.reader))
    }

    fn set_value(&mut self, property: &'static str, values: &'static [&'static str], raw: &str, start: usize) -> Result<(), CommandSyntaxException> {
        match property_value(values, raw) {
            Some(value) => {
                let state = self.state.expect("a parsed block");
                self.state = state.with_property_by_name(property, value);
                self.properties.push(property);
                Ok(())
            }
            None => {
                self.reader.cursor = start;
                Err(ERROR_INVALID_VALUE.create_with_context(self.reader, self.id.as_str(), property, raw))
            }
        }
    }
}
