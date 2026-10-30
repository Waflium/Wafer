use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use super::cached_user_name_to_id_resolver::{CachedUserNameToIdResolver, is_valid_player_name};
use crate::network::login::Profile;
use crate::server::services::Services;

const EXPIRE_AFTER_ACCESS: Duration = Duration::from_secs(10 * 60);
const MAXIMUM_SIZE: usize = 256;

struct LoadingCache<K, V> {
    entries: Mutex<HashMap<K, (V, Instant)>>,
}

impl<K: Eq + Hash + Clone, V: Clone> LoadingCache<K, V> {
    fn new() -> LoadingCache<K, V> {
        LoadingCache { entries: Mutex::new(HashMap::new()) }
    }

    fn get_unchecked(&self, key: &K, load: impl FnOnce() -> V) -> V {
        let now = Instant::now();
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, (_, accessed)| now.duration_since(*accessed) < EXPIRE_AFTER_ACCESS);
        if let Some((value, accessed)) = entries.get_mut(key) {
            *accessed = now;
            return value.clone();
        }
        drop(entries);
        let value = load();
        let mut entries = self.entries.lock().unwrap();
        if entries.len() >= MAXIMUM_SIZE
            && let Some(oldest) = entries.iter().min_by_key(|(_, (_, accessed))| *accessed).map(|(k, _)| k.clone())
        {
            entries.remove(&oldest);
        }
        entries.insert(key.clone(), (value.clone(), now));
        value
    }
}

pub struct ProfileResolver {
    profile_cache_by_name: LoadingCache<String, Option<Profile>>,
    profile_cache_by_id: LoadingCache<u128, Option<Profile>>,
}

impl Default for ProfileResolver {
    fn default() -> ProfileResolver {
        ProfileResolver { profile_cache_by_name: LoadingCache::new(), profile_cache_by_id: LoadingCache::new() }
    }
}

const LOADING_CACHE_GET_UNCHECKED: [&str; 8] = [
    "com.google.common.cache.LocalCache$LoadingValueReference.loadFuture(LocalCache.java:3556)",
    "com.google.common.cache.LocalCache$Segment.loadSync(LocalCache.java:2307)",
    "com.google.common.cache.LocalCache$Segment.lockedGetOrLoad(LocalCache.java:2180)",
    "com.google.common.cache.LocalCache$Segment.get(LocalCache.java:2071)",
    "com.google.common.cache.LocalCache.get(LocalCache.java:3985)",
    "com.google.common.cache.LocalCache.getOrLoad(LocalCache.java:4006)",
    "com.google.common.cache.LocalCache$LocalLoadingCache.get(LocalCache.java:4946)",
    "com.google.common.cache.LocalCache$LocalLoadingCache.getUnchecked(LocalCache.java:4953)",
];

const BY_NAME_LOAD: [&str; 2] = [
    "net.minecraft.server.players.ProfileResolver$Cached$2.load(ProfileResolver.java:54)",
    "net.minecraft.server.players.ProfileResolver$Cached$2.load(ProfileResolver.java:50)",
];

const BY_NAME_NESTED_LOAD: [&str; 4] = [
    "net.minecraft.server.players.ProfileResolver$Cached$2.lambda$load$0(ProfileResolver.java:55)",
    "java.base/java.util.Optional.flatMap(Optional.java:289)",
    "net.minecraft.server.players.ProfileResolver$Cached$2.load(ProfileResolver.java:55)",
    "net.minecraft.server.players.ProfileResolver$Cached$2.load(ProfileResolver.java:50)",
];

const BY_ID_LOAD: [&str; 2] = [
    "net.minecraft.server.players.ProfileResolver$Cached$1.load(ProfileResolver.java:42)",
    "net.minecraft.server.players.ProfileResolver$Cached$1.load(ProfileResolver.java:39)",
];

const FETCH_BY_NAME: &str = "net.minecraft.server.players.ProfileResolver$Cached.fetchByName(ProfileResolver.java:63)";
const FETCH_BY_ID: &str = "net.minecraft.server.players.ProfileResolver$Cached.fetchById(ProfileResolver.java:70)";

impl ProfileResolver {
    pub fn fetch_by_name(&self, services: &Services, name_to_id_cache: &CachedUserNameToIdResolver, name: &str, callers: &[&'static str]) -> Option<Profile> {
        if !is_valid_player_name(name) {
            return None;
        }
        self.profile_cache_by_name.get_unchecked(&name.to_string(), || {
            let outer: Vec<&'static str> = LOADING_CACHE_GET_UNCHECKED.into_iter().chain([FETCH_BY_NAME]).chain(callers.iter().copied()).collect();
            let name_frames: Vec<&'static str> = BY_NAME_LOAD.into_iter().chain(outer.iter().copied()).collect();
            name_to_id_cache.get(services, name, &name_frames).and_then(|name_and_id| {
                let nested: Vec<&'static str> = BY_NAME_NESTED_LOAD.into_iter().chain(outer.iter().copied()).collect();
                self.fetch_by_id_loading(services, name_and_id.id, &nested)
            })
        })
    }

    pub fn fetch_by_id(&self, services: &Services, id: u128, callers: &[&'static str]) -> Option<Profile> {
        let outer: Vec<&'static str> = [FETCH_BY_ID].into_iter().chain(callers.iter().copied()).collect();
        self.fetch_by_id_loading(services, id, &outer)
    }

    fn fetch_by_id_loading(&self, services: &Services, id: u128, outer: &[&'static str]) -> Option<Profile> {
        self.profile_cache_by_id.get_unchecked(&id, || {
            let frames: Vec<&'static str> = BY_ID_LOAD.into_iter().chain(LOADING_CACHE_GET_UNCHECKED).chain(outer.iter().copied()).collect();
            services.fetch_game_profile(id, &frames)
        })
    }
}
