use wafer_data::packets::play::serverbound;
use wafer_protocol::Reader;

use crate::network::chat::Component;
use crate::network::chat::last_seen_messages::Update;
use crate::network::chat::message_signature::MessageSignature;
use crate::network::chat::remote_chat_session;
use crate::network::connection::Packet;
use crate::network::hashed_stack::HashedStack;
use crate::nbt::{java_double_to_string, java_float_to_string, to_snbt};
use crate::util::java_collections::identity_hash_code;
use crate::world::item::read_slot;
use crate::world::level::block::entity::test_instance_block_entity;
use crate::world::level::storage::format_uuid;
use crate::world::entity::player::profile_public_key::read_byte_array;

const GAME: &str = "net.minecraft.network.protocol.game.";
const COMMON: &str = "net.minecraft.network.protocol.common.";
const PING: &str = "net.minecraft.network.protocol.ping.";

pub fn to_string(packet: &Packet) -> Option<String> {
    packet_to_string(packet.id, &mut packet.reader()).ok()
}

fn packet_to_string(id: i32, r: &mut Reader) -> anyhow::Result<String> {
    Ok(match id {
        serverbound::ACCEPT_TELEPORTATION => format!(
            "ServerboundAcceptTeleportationPacket[id={}, x={}, y={}, z={}, yRot={}, xRot={}]",
            r.varint()?,
            double(r.f64()?),
            double(r.f64()?),
            double(r.f64()?),
            float(r.f32()?),
            float(r.f32()?)
        ),
        serverbound::ATTACK => format!("ServerboundAttackPacket[entityId={}]", r.varint()?),
        serverbound::BLOCK_ENTITY_TAG_QUERY => object(GAME, "ServerboundBlockEntityTagQueryPacket"),
        serverbound::BUNDLE_ITEM_SELECTED => format!("ServerboundSelectBundleItemPacket[slotId={}, selectedItemIndex={}]", r.varint()?, r.varint()?),
        serverbound::CHANGE_DIFFICULTY => format!("ServerboundChangeDifficultyPacket[difficulty={}]", wrap(r.varint()?, &["PEACEFUL", "EASY", "NORMAL", "HARD"])),
        serverbound::CHANGE_GAME_MODE => format!("ServerboundChangeGameModePacket[mode={}]", zero(r.varint()?, &["SURVIVAL", "CREATIVE", "ADVENTURE", "SPECTATOR"])),
        serverbound::CHAT_ACK => format!("ServerboundChatAckPacket[offset={}]", r.varint()?),
        serverbound::CHAT_COMMAND => format!("ServerboundChatCommandPacket[command={}]", r.string(32767)?),
        serverbound::CHAT_COMMAND_SIGNED => {
            let command = r.string(32767)?;
            let time_stamp = instant(r.i64()?);
            let salt = r.i64()?;
            let mut entries = Vec::new();
            for _ in 0..r.varint()? {
                let name = r.string(16)?;
                entries.push(format!("Entry[name={name}, signature={:?}]", MessageSignature::read(r)?));
            }
            format!(
                "ServerboundChatCommandSignedPacket[command={command}, timeStamp={time_stamp}, salt={salt}, argumentSignatures=ArgumentSignatures[entries=[{}]], lastSeenMessages={}]",
                entries.join(", "),
                Update::read(r)?
            )
        }
        serverbound::CHAT => format!(
            "ServerboundChatPacket[message={}, timeStamp={}, salt={}, signature={}, lastSeenMessages={}]",
            r.string(256)?,
            instant(r.i64()?),
            r.i64()?,
            optional(MessageSignature::read_optional(r)?.map(|signature| format!("{signature:?}"))),
            Update::read(r)?
        ),
        serverbound::CHAT_SESSION_UPDATE => format!("ServerboundChatSessionUpdatePacket[chatSession={}]", remote_chat_session::Data::read(r)?),
        serverbound::CHUNK_BATCH_RECEIVED => format!("ServerboundChunkBatchReceivedPacket[desiredChunksPerTick={}]", float(r.f32()?)),
        serverbound::CLIENT_COMMAND => object(GAME, "ServerboundClientCommandPacket"),
        serverbound::CLIENT_TICK_END => "ServerboundClientTickEndPacket[]".to_string(),
        serverbound::CLIENT_INFORMATION => format!(
            "ServerboundClientInformationPacket[information=ClientInformation[language={}, viewDistance={}, chatVisibility={}, chatColors={}, modelCustomisation={}, mainHand={}, textFilteringEnabled={}, allowsListing={}, particleStatus={}]]",
            r.string(16)?,
            r.u8()? as i8,
            wrap(r.varint()?, &["FULL", "SYSTEM", "HIDDEN"]),
            r.bool()?,
            r.u8()?,
            zero(r.varint()?, &["LEFT", "RIGHT"]),
            r.bool()?,
            r.bool()?,
            wrap(r.varint()?, &["ALL", "DECREASED", "MINIMAL"])
        ),
        serverbound::CONFIGURATION_ACKNOWLEDGED => object(GAME, "ServerboundConfigurationAcknowledgedPacket"),
        serverbound::CONTAINER_BUTTON_CLICK => format!("ServerboundContainerButtonClickPacket[containerId={}, buttonId={}]", r.varint()?, r.varint()?),
        serverbound::CONTAINER_CLICK => {
            let container_id = r.varint()?;
            let state_id = r.varint()?;
            let slot_num = r.i16()?;
            let button_num = r.u8()? as i8;
            let container_input = zero(r.varint()?, &["PICKUP", "QUICK_MOVE", "SWAP", "CLONE", "THROW", "QUICK_CRAFT", "PICKUP_ALL"]);
            let count = r.varint()?;
            let mut changed_slots = Int2ObjectOpenHashMap::new(count.min(65536));
            for _ in 0..count {
                let slot = r.i16()? as i32;
                changed_slots.put(slot, HashedStack::read(r)?.to_string());
            }
            format!(
                "ServerboundContainerClickPacket[containerId={container_id}, stateId={state_id}, slotNum={slot_num}, buttonNum={button_num}, containerInput={container_input}, changedSlots={changed_slots}, carriedItem={}]",
                HashedStack::read(r)?
            )
        }
        serverbound::CONTAINER_CLOSE => object(GAME, "ServerboundContainerClosePacket"),
        serverbound::CONTAINER_SLOT_STATE_CHANGED => format!("ServerboundContainerSlotStateChangedPacket[slotId={}, containerId={}, newState={}]", r.varint()?, r.varint()?, r.bool()?),
        serverbound::COOKIE_RESPONSE => format!(
            "ServerboundCookieResponsePacket[key={}, payload={}]",
            identifier(r)?,
            if r.bool()? {
                read_byte_array(r, 5120)?;
                object("", "[B")
            } else {
                "null".to_string()
            }
        ),
        serverbound::CUSTOM_PAYLOAD => {
            let id = identifier(r)?;
            let payload = if id == "minecraft:brand" { format!("BrandPayload[brand={}]", r.string(32767)?) } else { format!("DiscardedPayload[id={id}]") };
            format!("ServerboundCustomPayloadPacket[payload={payload}]")
        }
        serverbound::DEBUG_SUBSCRIPTION_REQUEST => {
            let mut subscriptions = Vec::new();
            for _ in 0..r.varint()? {
                let id = r.varint()?;
                let name = usize::try_from(id).ok().and_then(|id| wafer_data::registry::DEBUG_SUBSCRIPTION.get(id)).ok_or_else(|| anyhow::anyhow!("Unknown debug subscription {id}"))?;
                if !subscriptions.contains(name) {
                    subscriptions.push(*name);
                }
            }
            format!("ServerboundDebugSubscriptionRequestPacket[subscriptions={{{}}}]", subscriptions.join(", "))
        }
        serverbound::EDIT_BOOK => {
            let slot = r.varint()?;
            let mut pages = Vec::new();
            for _ in 0..r.varint()? {
                pages.push(r.string(1024)?);
            }
            let title = if r.bool()? { Some(r.string(32)?) } else { None };
            format!("ServerboundEditBookPacket[slot={slot}, pages=[{}], title={}]", pages.join(", "), optional(title))
        }
        serverbound::ENTITY_TAG_QUERY => object(GAME, "ServerboundEntityTagQueryPacket"),
        serverbound::INTERACT => {
            let entity_id = r.varint()?;
            let hand = hand(r.varint()?);
            let (x, y, z) = r.lp_vec3()?;
            format!("ServerboundInteractPacket[entityId={entity_id}, hand={hand}, location={}, usingSecondaryAction={}]", vec3(x, y, z), r.bool()?)
        }
        serverbound::JIGSAW_GENERATE => object(GAME, "ServerboundJigsawGeneratePacket"),
        serverbound::KEEP_ALIVE => object(COMMON, "ServerboundKeepAlivePacket"),
        serverbound::LOCK_DIFFICULTY => object(GAME, "ServerboundLockDifficultyPacket"),
        serverbound::MOVE_PLAYER_POS => object(GAME, "ServerboundMovePlayerPacket$Pos"),
        serverbound::MOVE_PLAYER_POS_ROT => object(GAME, "ServerboundMovePlayerPacket$PosRot"),
        serverbound::MOVE_PLAYER_ROT => object(GAME, "ServerboundMovePlayerPacket$Rot"),
        serverbound::MOVE_PLAYER_STATUS_ONLY => object(GAME, "ServerboundMovePlayerPacket$StatusOnly"),
        serverbound::MOVE_VEHICLE => format!(
            "ServerboundMoveVehiclePacket[movingTo=Immutable[position={}, yRot={}, xRot={}], onGround={}]",
            vec3(r.f64()?, r.f64()?, r.f64()?),
            float(r.f32()?),
            float(r.f32()?),
            r.bool()?
        ),
        serverbound::PADDLE_BOAT => object(GAME, "ServerboundPaddleBoatPacket"),
        serverbound::PICK_ITEM_FROM_BLOCK => format!("ServerboundPickItemFromBlockPacket[pos={}, includeData={}]", block_pos(r)?, r.bool()?),
        serverbound::PICK_ITEM_FROM_ENTITY => format!("ServerboundPickItemFromEntityPacket[id={}, includeData={}]", r.varint()?, r.bool()?),
        serverbound::PING_REQUEST => object(PING, "ServerboundPingRequestPacket"),
        serverbound::PLACE_RECIPE => format!("ServerboundPlaceRecipePacket[containerId={}, recipe=RecipeDisplayId[index={}], useMaxItems={}]", r.varint()?, r.varint()?, r.bool()?),
        serverbound::PLAYER_ABILITIES => object(GAME, "ServerboundPlayerAbilitiesPacket"),
        serverbound::PLAYER_ACTION => object(GAME, "ServerboundPlayerActionPacket"),
        serverbound::PLAYER_COMMAND => object(GAME, "ServerboundPlayerCommandPacket"),
        serverbound::PLAYER_INPUT => {
            let flags = r.u8()?;
            let flag = |bit: u8| flags & bit != 0;
            format!(
                "ServerboundPlayerInputPacket[input=Input[forward={}, backward={}, left={}, right={}, jump={}, shift={}, sprint={}]]",
                flag(1),
                flag(2),
                flag(4),
                flag(8),
                flag(16),
                flag(32),
                flag(64)
            )
        }
        serverbound::PLAYER_LOADED => "ServerboundPlayerLoadedPacket[]".to_string(),
        serverbound::PONG => object(COMMON, "ServerboundPongPacket"),
        serverbound::PUNCH => "ServerboundPunchPacket[]".to_string(),
        serverbound::RECIPE_BOOK_CHANGE_SETTINGS => object(GAME, "ServerboundRecipeBookChangeSettingsPacket"),
        serverbound::RECIPE_BOOK_SEEN_RECIPE => format!("ServerboundRecipeBookSeenRecipePacket[recipe=RecipeDisplayId[index={}]]", r.varint()?),
        serverbound::RENAME_ITEM => object(GAME, "ServerboundRenameItemPacket"),
        serverbound::RESOURCE_PACK => {
            let id = format_uuid(r.uuid()?);
            let action = r.varint()?;
            let action = usize::try_from(action)
                .ok()
                .and_then(|action| ["SUCCESSFULLY_LOADED", "DECLINED", "FAILED_DOWNLOAD", "ACCEPTED", "DOWNLOADED", "INVALID_URL", "FAILED_RELOAD", "DISCARDED"].get(action))
                .ok_or_else(|| anyhow::anyhow!("Unknown resource pack action {action}"))?;
            format!("ServerboundResourcePackPacket[id={id}, action={action}]")
        }
        serverbound::SEEN_ADVANCEMENTS => object(GAME, "ServerboundSeenAdvancementsPacket"),
        serverbound::SELECT_TRADE => object(GAME, "ServerboundSelectTradePacket"),
        serverbound::SET_BEACON => format!("ServerboundSetBeaconPacket[primary={}, secondary={}]", optional(mob_effect(r)?), optional(mob_effect(r)?)),
        serverbound::SET_CARRIED_ITEM => object(GAME, "ServerboundSetCarriedItemPacket"),
        serverbound::SET_COMMAND_BLOCK => object(GAME, "ServerboundSetCommandBlockPacket"),
        serverbound::SET_COMMAND_MINECART => object(GAME, "ServerboundSetCommandMinecartPacket"),
        serverbound::SET_CREATIVE_MODE_SLOT => {
            let slot_num = r.i16()?;
            let item_stack = read_slot(r, true).map_err(anyhow::Error::msg)?.map_or_else(|| "0 minecraft:air".to_string(), |stack| format!("{} {}", stack.count, stack.item.name()));
            format!("ServerboundSetCreativeModeSlotPacket[slotNum={slot_num}, itemStack={item_stack}]")
        }
        serverbound::SET_GAME_RULE => {
            let mut entries = Vec::new();
            for _ in 0..r.varint()? {
                let game_rule_key = identifier(r)?;
                entries.push(format!("Entry[gameRuleKey=ResourceKey[minecraft:game_rule / {game_rule_key}], value={}]", r.string(32767)?));
            }
            format!("ServerboundSetGameRulePacket[entries=[{}]]", entries.join(", "))
        }
        serverbound::SET_JIGSAW_BLOCK => object(GAME, "ServerboundSetJigsawBlockPacket"),
        serverbound::SET_STRUCTURE_BLOCK => object(GAME, "ServerboundSetStructureBlockPacket"),
        serverbound::SET_TEST_BLOCK => format!("ServerboundSetTestBlockPacket[position={}, mode={}, message={}]", block_pos(r)?, zero(r.varint()?, &["START", "LOG", "FAIL", "ACCEPT"]), r.string(32767)?),
        serverbound::SIGN_UPDATE => {
            let pos = block_pos(r)?;
            let mut lines = Vec::with_capacity(4);
            for _ in 0..4 {
                lines.push(r.string(384)?);
            }
            format!("ServerboundSignUpdatePacket[pos={pos}, lines=[{}], slot={}]", lines.join(", "), zero(r.varint()?, &["BACK", "FRONT"]))
        }
        serverbound::SPECTATOR_ACTION => {
            let spectate_entity_id = match r.varint()? {
                0 => "OptionalInt.empty".to_string(),
                id => format!("OptionalInt[{}]", id - 1),
            };
            format!("ServerboundSpectatorActionPacket[spectateEntityId={spectate_entity_id}]")
        }
        serverbound::TELEPORT_TO_ENTITY => object(GAME, "ServerboundTeleportToEntityPacket"),
        serverbound::TEST_INSTANCE_BLOCK_ACTION => format!(
            "ServerboundTestInstanceBlockActionPacket[pos={}, action={}, data={}]",
            block_pos(r)?,
            zero(r.varint()?, &["INIT", "QUERY", "SET", "RESET", "SAVE", "EXPORT", "RUN"]),
            test_instance_block_entity::Data::read(r)?
        ),
        serverbound::USE_ITEM_ON => format!("ServerboundUseItemOnPacket[hand={}, hitResult={}, sequence={}]", hand(r.varint()?), skip_block_hit_result(r)?, r.varint()?),
        serverbound::USE_ITEM => format!("ServerboundUseItemPacket[hand={}, sequence={}, yRot={}, xRot={}]", hand(r.varint()?), r.varint()?, float(r.f32()?), float(r.f32()?)),
        serverbound::CUSTOM_CLICK_ACTION => {
            let id = identifier(r)?;
            let length = r.varint()?;
            let tag = r.take(usize::try_from(length)?)?;
            let payload = if tag.first() == Some(&0) { None } else { Some(to_snbt(&wafer_protocol::nbt::Nbt::read(&mut Reader::new(tag))?)) };
            format!("ServerboundCustomClickActionPacket[id={id}, payload={}]", optional(payload))
        }
        _ => anyhow::bail!("Unknown packet {id}"),
    })
}

fn object(package: &str, class: &str) -> String {
    format!("{package}{class}@{:x}", identity_hash_code())
}

fn double(value: f64) -> String {
    java_double_to_string(value)
}

fn float(value: f32) -> String {
    java_float_to_string(value)
}

fn vec3(x: f64, y: f64, z: f64) -> String {
    format!("({}, {}, {})", double(x), double(y), double(z))
}

fn block_pos(r: &mut Reader) -> anyhow::Result<wafer_core::BlockPos> {
    let (x, y, z) = r.position()?;
    Ok(wafer_core::BlockPos::new(x, y, z))
}

fn identifier(r: &mut Reader) -> anyhow::Result<String> {
    crate::resources::identifier::read(r.string(32767)?).map_err(anyhow::Error::msg)
}

fn optional<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "Optional.empty".to_string(), |value| format!("Optional[{value}]"))
}

fn wrap(id: i32, names: &[&'static str]) -> &'static str {
    names[id.rem_euclid(names.len() as i32) as usize]
}

fn zero(id: i32, names: &[&'static str]) -> &'static str {
    usize::try_from(id).ok().and_then(|id| names.get(id)).copied().unwrap_or(names[0])
}

fn hand(id: i32) -> &'static str {
    zero(id, &["MAIN_HAND", "OFF_HAND"])
}

fn skip_block_hit_result(r: &mut Reader) -> anyhow::Result<String> {
    r.position()?;
    r.varint()?;
    r.f32()?;
    r.f32()?;
    r.f32()?;
    r.bool()?;
    r.bool()?;
    Ok(object("net.minecraft.world.phys.", "BlockHitResult"))
}

fn mob_effect(r: &mut Reader) -> anyhow::Result<Option<String>> {
    if !r.bool()? {
        return Ok(None);
    }
    let id = r.varint()?;
    let effect = usize::try_from(id).ok().and_then(|id| wafer_data::MOB_EFFECTS.get(id)).ok_or_else(|| anyhow::anyhow!("Unknown mob effect id {id}"))?;
    let class = match effect.name {
        "minecraft:instant_health" | "minecraft:instant_damage" => "HealOrHarmMobEffect",
        "minecraft:regeneration" => "RegenerationMobEffect",
        "minecraft:hunger" => "HungerMobEffect",
        "minecraft:poison" => "PoisonMobEffect",
        "minecraft:wither" => "WitherMobEffect",
        "minecraft:absorption" => "AbsorptionMobEffect",
        "minecraft:saturation" => "SaturationMobEffect",
        "minecraft:bad_omen" => "BadOmenMobEffect",
        "minecraft:raid_omen" => "RaidOmenMobEffect",
        "minecraft:wind_charged" => "WindChargedMobEffect",
        "minecraft:weaving" => "WeavingMobEffect",
        "minecraft:oozing" => "OozingMobEffect",
        "minecraft:infested" => "InfestedMobEffect",
        _ => "MobEffect",
    };
    Ok(Some(format!("Reference{{ResourceKey[minecraft:mob_effect / {}]={}}}", effect.name, object("net.minecraft.world.effect.", class))))
}

fn instant(epoch_milli: i64) -> String {
    const SECONDS_PER_10000_YEARS: i64 = 146097 * 25 * 86400;
    const SECONDS_0000_TO_1970: i64 = (146097 * 5 - (30 * 365 + 7)) * 86400;
    let in_secs = epoch_milli.div_euclid(1000);
    let in_milli = epoch_milli.rem_euclid(1000);
    let mut out = String::new();
    if in_secs >= -SECONDS_0000_TO_1970 {
        let zero_secs = in_secs - SECONDS_PER_10000_YEARS + SECONDS_0000_TO_1970;
        let hi = zero_secs.div_euclid(SECONDS_PER_10000_YEARS) + 1;
        let lo = zero_secs.rem_euclid(SECONDS_PER_10000_YEARS);
        let (year, rest) = local_date_time(lo - SECONDS_0000_TO_1970);
        if hi > 0 {
            out.push('+');
            out.push_str(&hi.to_string());
        }
        out.push_str(&format_year(year));
        out.push_str(&rest);
    } else {
        let zero_secs = in_secs + SECONDS_0000_TO_1970;
        let hi = zero_secs / SECONDS_PER_10000_YEARS;
        let lo = zero_secs % SECONDS_PER_10000_YEARS;
        let (year, rest) = local_date_time(lo - SECONDS_0000_TO_1970);
        let year_text = format_year(year);
        if hi < 0 {
            if year == -10000 {
                out.push_str(&(hi - 1).to_string());
                out.push_str(&year_text[2..]);
            } else if lo == 0 {
                out.push_str(&hi.to_string());
                out.push_str(&year_text);
            } else {
                out.push_str(&year_text[..1]);
                out.push_str(&hi.abs().to_string());
                out.push_str(&year_text[1..]);
            }
        } else {
            out.push_str(&year_text);
        }
        out.push_str(&rest);
    }
    if in_milli > 0 {
        out.push_str(&format!(".{in_milli:03}"));
    }
    out.push('Z');
    out
}

fn format_year(year: i64) -> String {
    if year.abs() < 1000 {
        if year < 0 { format!("-{:04}", -year) } else { format!("{year:04}") }
    } else if year > 9999 {
        format!("+{year}")
    } else {
        year.to_string()
    }
}

fn local_date_time(epoch_second: i64) -> (i64, String) {
    let days = epoch_second.div_euclid(86400);
    let secs = epoch_second.rem_euclid(86400);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, format!("-{month:02}-{day:02}T{:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60))
}

struct Int2ObjectOpenHashMap {
    keys: Vec<i32>,
    values: Vec<Option<String>>,
    null_value: Option<String>,
}

impl Int2ObjectOpenHashMap {
    fn new(expected: i32) -> Int2ObjectOpenHashMap {
        let n = ((expected as f32 / 0.75).ceil() as u64).max(1).next_power_of_two().max(2) as usize;
        Int2ObjectOpenHashMap { keys: vec![0; n], values: vec![None; n], null_value: None }
    }

    fn put(&mut self, key: i32, value: String) {
        if key == 0 {
            self.null_value = Some(value);
            return;
        }
        let mask = self.keys.len() - 1;
        let h = key.wrapping_mul(0x9E3779B9u32 as i32);
        let mut pos = (h ^ ((h as u32) >> 16) as i32) as usize & mask;
        while self.values[pos].is_some() && self.keys[pos] != key {
            pos = (pos + 1) & mask;
        }
        self.keys[pos] = key;
        self.values[pos] = Some(value);
    }
}

impl std::fmt::Display for Int2ObjectOpenHashMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let entries = self.null_value.iter().map(|value| (0, value)).chain((0..self.keys.len()).rev().filter_map(|pos| self.values[pos].as_ref().map(|value| (self.keys[pos], value))));
        f.write_str("{")?;
        for (i, (key, value)) in entries.enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{key}=>{value}")?;
        }
        f.write_str("}")
    }
}

impl std::fmt::Display for Update {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let acknowledged: Vec<String> = (0..32).filter(|bit| self.acknowledged >> bit & 1 != 0).map(|bit| bit.to_string()).collect();
        write!(f, "Update[offset={}, acknowledged={{{}}}, checksum={}]", self.offset, acknowledged.join(", "), self.checksum as i8)
    }
}

impl std::fmt::Display for remote_chat_session::Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use rsa::traits::PublicKeyParts;
        let key = &self.profile_public_key.key;
        write!(
            f,
            "Data[sessionId={}, profilePublicKey=Data[expiresAt={}, key=Sun RSA public key, {} bits\n  params: null\n  modulus: {}\n  public exponent: {}, keySignature={}]]",
            format_uuid(self.session_id),
            instant(self.profile_public_key.expires_at),
            key.n().bits(),
            key.n(),
            key.e(),
            object("", "[B")
        )
    }
}

impl std::fmt::Display for HashedStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashedStack::Empty => f.write_str("<empty>"),
            HashedStack::ActualItem { item, count, components } => {
                let added: Vec<String> = components.added_components.iter().map(|(ty, hash)| format!("{}={hash}", ty.name())).collect();
                let removed: Vec<&str> = components.removed_components.iter().map(|ty| ty.name()).collect();
                write!(
                    f,
                    "ActualItem[item=Reference{{ResourceKey[minecraft:item / {name}]={name}}}, count={count}, components=HashedPatchMap[addedComponents={{{}}}, removedComponents=[{}]]]",
                    added.join(", "),
                    removed.join(", "),
                    name = item.name()
                )
            }
        }
    }
}

impl std::fmt::Display for test_instance_block_entity::Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::world::level::block::rotation::Rotation;
        use test_instance_block_entity::Status;
        let rotation = match self.rotation {
            Rotation::None => "NONE",
            Rotation::Clockwise90 => "CLOCKWISE_90",
            Rotation::Clockwise180 => "CLOCKWISE_180",
            Rotation::Counterclockwise90 => "COUNTERCLOCKWISE_90",
        };
        let status = match self.status {
            Status::Cleared => "CLEARED",
            Status::Running => "RUNNING",
            Status::Finished => "FINISHED",
        };
        write!(
            f,
            "Data[test={}, size=Vec3i{{x={}, y={}, z={}}}, rotation={rotation}, ignoreEntities={}, status={status}, errorMessage={}]",
            optional(self.test.as_ref().map(|test| format!("ResourceKey[minecraft:test_instance / {test}]"))),
            self.size.x,
            self.size.y,
            self.size.z,
            self.ignore_entities,
            optional(self.error_message.as_ref().map(Component::java_to_string))
        )
    }
}
