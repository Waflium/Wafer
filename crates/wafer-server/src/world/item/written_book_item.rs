use wafer_data::packets::play::clientbound;
use wafer_protocol::nbt::Nbt;

use crate::commands::source::CommandSourceStack;
use crate::network::chat::Component;
use crate::network::chat::component_utils;
use crate::server::Server;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::swing_animation::SwingAnimation;
use crate::world::{InteractionHand, InteractionResult};

const MAX_PAGE_JSON_LENGTH: usize = 32767;

pub struct PendingBookOpen {
    pub player: EntityId,
    pub hand: InteractionHand,
    pub swing: Option<(SwingAnimation, bool)>,
}

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
    if item_stack.has(dc::WRITTEN_BOOK_CONTENT) {
        level.pending_book_opens.push(PendingBookOpen { player, hand, swing: None });
    }
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item_stack.item.0), 1);
    (InteractionResult::SUCCESS, None)
}

fn resolve_page(server: &Server, source: &CommandSourceStack, page: &Nbt) -> Option<Nbt> {
    let component = Component::from_nbt(page);
    match component_utils::resolve(server, source, &component) {
        Ok(resolved) if resolved.to_json().len() > MAX_PAGE_JSON_LENGTH => None,
        Ok(resolved) => Some(resolved.to_nbt()),
        Err(_) => Some(page.clone()),
    }
}

fn resolve(server: &Server, source: &CommandSourceStack, content: &[(String, Nbt)]) -> Option<Vec<(String, Nbt)>> {
    let mut pages = Vec::new();
    for page in content.iter().find(|(k, _)| k == "pages").map_or(&[][..], |(_, pages)| pages.as_list()) {
        let Nbt::Compound(fields) = page else { return None };
        let mut resolved_page = Vec::with_capacity(fields.len());
        for (key, value) in fields {
            resolved_page.push((key.clone(), if key == "raw" || key == "filtered" { resolve_page(server, source, value)? } else { value.clone() }));
        }
        pages.push(Nbt::Compound(resolved_page));
    }
    let mut resolved = content.to_vec();
    resolved.retain(|(k, _)| k != "pages" && k != "resolved");
    if !pages.is_empty() {
        resolved.push(("pages".into(), Nbt::List(pages)));
    }
    resolved.push(("resolved".into(), Nbt::Byte(1)));
    Some(resolved)
}

fn resolve_for_item(server: &Server, source: &CommandSourceStack, item_stack: &mut ItemStack) -> bool {
    let Some(Nbt::Compound(content)) = item_stack.get(dc::WRITTEN_BOOK_CONTENT) else { return false };
    if content.iter().find(|(k, _)| k == "resolved").and_then(|(_, v)| v.as_i64()).is_some_and(|resolved| resolved != 0) {
        return false;
    }
    match resolve(server, source, content) {
        Some(resolved) => {
            item_stack.set(dc::WRITTEN_BOOK_CONTENT, Some(Nbt::Compound(resolved)));
            true
        }
        None => {
            crate::world::item::component::values::WrittenBookContent::resolve_for_item(item_stack);
            false
        }
    }
}

impl Server {
    pub fn open_pending_books(&mut self, level: usize) {
        for PendingBookOpen { player, hand, swing } in std::mem::take(&mut self.levels[level].pending_book_opens) {
            if let Some(mut item_stack) = self.levels[level].players.get(&player).and_then(|p| p.item_in_hand(hand))
                && item_stack.has(dc::WRITTEN_BOOK_CONTENT)
            {
                let source = CommandSourceStack::player(self, player);
                let changed = resolve_for_item(self, &source, &mut item_stack);
                let level = &mut self.levels[level];
                level.player_mut(player).update_item_in_hand(hand, &item_stack);
                if changed {
                    level.broadcast_container_menu_changes(player);
                }
                level.player(player).connection.send(clientbound::OPEN_BOOK, |w| w.varint(hand as i32));
            }
            if let Some((animation, send_to_swinging_entity)) = swing {
                self.levels[level].swing_and_reset_attack_strength(player, hand, animation, send_to_swinging_entity);
            }
        }
    }
}
