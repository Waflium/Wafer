use std::sync::Arc;

use super::cached_user_name_to_id_resolver::CachedUserNameToIdResolver;
use crate::commands::arguments::java_uuid_from_string;
use crate::network::login::offline_uuid;
use crate::server::services::Services;

#[derive(Clone)]
pub struct UserLookup {
    pub name_to_id_cache: Arc<CachedUserNameToIdResolver>,
    pub services: Services,
    pub uses_authentication: bool,
}

pub fn convert_mob_owner_if_necessary(lookup: &UserLookup, owner: &str) -> Option<u128> {
    if !owner.is_empty() && owner.encode_utf16().count() <= 16 {
        if let Some(profile) = lookup.name_to_id_cache.get(&lookup.services, owner) {
            Some(profile.id)
        } else if lookup.uses_authentication {
            let Some(profile) = lookup.services.find_profile_by_name(owner) else {
                warn!("Could not lookup user whitelist entry for {owner}");
                return None;
            };
            lookup.name_to_id_cache.add(profile.clone());
            Some(profile.id)
        } else {
            Some(offline_uuid(owner))
        }
    } else {
        java_uuid_from_string(owner)
    }
}
