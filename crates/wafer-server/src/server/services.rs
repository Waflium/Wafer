use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use base64::Engine;
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;

use crate::server::players::NameAndId;
use crate::server::players::old_users_converter::{ConversionError, ProfileLookupCallback};
use crate::util::java_collections::JavaHashSet;
use crate::util::java_throwable::JavaThrowable;
use crate::util::signature_validator::SignatureValidator;
use crate::util::gson::json_reader::{JsonReader, JsonToken, Strictness};

const DISCOVERY_URL: &str = "https://discovery.minecraftservices.com/minecraft/client";
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_BACKOFF_EXPONENT: u32 = 6;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
const READ_TIMEOUT: Duration = Duration::from_millis(5100);

pub type Callers<'a> = &'a [&'static str];

struct Fetched<T> {
    value: Mutex<Option<Arc<T>>>,
    ready: (Mutex<bool>, Condvar),
    attempt: tokio::sync::Notify,
}

impl<T> Fetched<T> {
    fn new(ready: bool) -> Fetched<T> {
        Fetched { value: Mutex::new(None), ready: (Mutex::new(ready), Condvar::new()), attempt: tokio::sync::Notify::new() }
    }

    async fn get_async(&self) -> Option<Arc<T>> {
        loop {
            let attempt = self.attempt.notified();
            if *self.ready.0.lock().unwrap() {
                return self.value.lock().unwrap().clone();
            }
            attempt.await;
        }
    }

    fn get(&self) -> Option<Arc<T>> {
        let (lock, condvar) = &self.ready;
        drop(condvar.wait_while(lock.lock().unwrap(), |ready| !*ready).unwrap());
        self.value.lock().unwrap().clone()
    }

    fn attempted(&self, value: Option<T>) {
        if let Some(value) = value {
            *self.value.lock().unwrap() = Some(Arc::new(value));
        }
        let (lock, condvar) = &self.ready;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        self.attempt.notify_waiters();
    }

    fn has_value(&self) -> bool {
        self.value.lock().unwrap().is_some()
    }
}

fn retryable_fetch<T: Send + Sync + 'static, F>(target: Arc<Fetched<T>>, retry: Duration, fetch: impl Fn() -> F + Send + 'static)
where
    F: Future<Output = Option<T>> + Send,
{
    tokio::spawn(async move {
        let mut failures = 0;
        loop {
            target.attempted(fetch().await);
            let delay = if target.has_value() {
                failures = 0;
                REFRESH_INTERVAL
            } else {
                let delay = retry * (1 << failures.min(MAX_BACKOFF_EXPONENT));
                failures += 1;
                delay
            };
            tokio::time::sleep(delay).await;
        }
    });
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ErrorType {
    ServiceUnavailable,
    HttpError,
    JsonError,
}

#[derive(Clone, Debug)]
pub struct MinecraftClientException {
    error_type: ErrorType,
    message: String,
    throwable: JavaThrowable,
    http: Option<(u16, String)>,
    html_body_log: Option<String>,
}

const TO_AUTHENTICATION_EXCEPTION: &str = "com.mojang.authlib.exceptions.MinecraftClientException.toAuthenticationException(MinecraftClientException.java:29)";
const TO_AUTHENTICATION_EXCEPTION_JSON: &str = "com.mojang.authlib.exceptions.MinecraftClientException.toAuthenticationException(MinecraftClientException.java:31)";

fn http_to_authentication_exception(line: u32) -> String {
    format!("com.mojang.authlib.exceptions.MinecraftClientHttpException.toAuthenticationException(MinecraftClientHttpException.java:{line})")
}

impl MinecraftClientException {
    pub fn to_authentication_exception(&self, callers: Callers) -> JavaThrowable {
        let leak = |line: u32| -> &'static str { Box::leak(http_to_authentication_exception(line).into_boxed_str()) };
        let build = |description: String, frame: &'static str, cause: Option<&MinecraftClientException>| {
            let mut throwable = JavaThrowable::new(description, vec![frame]);
            throwable.push_frames(callers);
            match cause {
                Some(cause) => throwable.with_cause(cause.throwable.clone()),
                None => throwable,
            }
        };
        match (&self.error_type, &self.http) {
            (ErrorType::ServiceUnavailable, _) => build("com.mojang.authlib.exceptions.AuthenticationUnavailableException".to_string(), TO_AUTHENTICATION_EXCEPTION, None),
            (ErrorType::JsonError, _) | (ErrorType::HttpError, None) => {
                build(format!("com.mojang.authlib.exceptions.AuthenticationException: {}", self.throwable.description), TO_AUTHENTICATION_EXCEPTION_JSON, Some(self))
            }
            (ErrorType::HttpError, Some((status, error))) => {
                let has_error = |name: &str| error.eq_ignore_ascii_case(name);
                if has_error("ForbiddenOperationException") {
                    build(format!("com.mojang.authlib.exceptions.InvalidCredentialsException: {}", self.message), leak(60), None)
                } else if has_error("multiplayer.access.banned") {
                    build("com.mojang.authlib.exceptions.UserBannedException".to_string(), leak(62), None)
                } else if has_error("FORCED_USERNAME_CHANGE") {
                    build("com.mojang.authlib.exceptions.ForcedUsernameChangeException".to_string(), leak(64), None)
                } else if has_error("InsufficientPrivilegesException") {
                    build(format!("com.mojang.authlib.exceptions.InsufficientPrivilegesException: {}", self.message), leak(66), Some(self))
                } else if *status == 401 {
                    build(format!("com.mojang.authlib.exceptions.InvalidCredentialsException: {}", self.message), leak(70), Some(self))
                } else if *status >= 500 {
                    build(format!("com.mojang.authlib.exceptions.AuthenticationUnavailableException: {}", self.message), leak(74), Some(self))
                } else {
                    build(format!("com.mojang.authlib.exceptions.AuthenticationException: {}", self.message), leak(77), Some(self))
                }
            }
        }
    }

    pub fn log_html_body(&self, thread: Option<&str>) {
        if let Some(message) = &self.html_body_log {
            match thread {
                Some(thread) => crate::logging::log_as(thread, crate::logging::Level::Error, format_args!("{message}")),
                None => crate::logging::log(crate::logging::Level::Error, format_args!("{message}")),
            }
        }
    }
}

struct ServicesKeySet {
    profile_keys: Vec<RsaPublicKey>,
}

#[derive(Clone)]
pub struct Services {
    discovery: Arc<Fetched<serde_json::Value>>,
    key_set: Arc<Fetched<ServicesKeySet>>,
    http: Option<(reqwest::Client, tokio::runtime::Handle)>,
}

enum UrlFailure {
    Offline,
    MissingEndpoint,
}

impl Services {
    pub fn create() -> Services {
        info!("Environment: Environment[discoveryUrl={DISCOVERY_URL}, name=PROD]");
        let client = reqwest::Client::builder().connector_layer(tower::timeout::TimeoutLayer::new(CONNECT_TIMEOUT)).read_timeout(READ_TIMEOUT).build().expect("an HTTP client");
        let services = Services {
            discovery: Arc::new(Fetched::new(false)),
            key_set: Arc::new(Fetched::new(false)),
            http: Some((client.clone(), tokio::runtime::Handle::current())),
        };
        let http = client.clone();
        retryable_fetch(services.discovery.clone(), Duration::from_secs(60), move || {
            let http = http.clone();
            async move { get_json(&http, DISCOVERY_URL).await }
        });
        let this = services.clone();
        retryable_fetch(services.key_set.clone(), Duration::from_secs(5 * 60), move || {
            let (http, this) = (client.clone(), this.clone());
            async move {
                let url = match this.url_async_on("authentication", "getPublicKeys", Some(DISCOVERY_THREAD)).await {
                    Ok(url) => url,
                    Err(failure) => {
                        warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", services_unavailable(&failure, &KEY_UNAVAILABLE_CONTEXT).throwable.print_stack_trace().trim_end());
                        if let Err(failure) = this.url_async_on("authentication", "getPublicKeys", Some(DISCOVERY_THREAD)).await {
                            warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", services_unavailable(&failure, &KEY_UNAVAILABLE_CONTEXT).throwable.print_stack_trace().trim_end());
                        }
                        return None;
                    }
                };
                let response = match client_get(&http, &url, &KEY_FETCH_CONTEXT).await {
                    Ok(response) => response?,
                    Err(exception) => {
                        for _ in 0..2 {
                            exception.log_html_body(Some(DISCOVERY_THREAD));
                            error!(thread: DISCOVERY_THREAD, "Failed to request Minecraft Services public key\n{}", exception.throwable.print_stack_trace().trim_end());
                        }
                        return None;
                    }
                };
                let keys = response["playerCertificateKeys"].as_array().map(Vec::as_slice).unwrap_or_default();
                let profile_keys = keys.iter().map(|k| parse_key(k["publicKey"].as_str()?)).collect::<Option<Vec<_>>>();
                if profile_keys.is_none() {
                    error!(thread: DISCOVERY_THREAD, "Received malformed Minecraft Services public key data");
                }
                Some(ServicesKeySet { profile_keys: profile_keys? })
            }
        });
        services
    }

    pub fn offline() -> Services {
        Services { discovery: Arc::new(Fetched::new(true)), key_set: Arc::new(Fetched::new(true)), http: None }
    }

    pub async fn url_async(&self, service: &str, endpoint: &str) -> Option<String> {
        self.url_async_on(service, endpoint, None).await.ok()
    }

    async fn url_async_on(&self, service: &str, endpoint: &str, thread: Option<&str>) -> Result<String, UrlFailure> {
        let service_name = service.to_uppercase();
        let log = |message: String| match thread {
            Some(thread) => crate::logging::log_as(thread, crate::logging::Level::Warn, format_args!("{message}")),
            None => crate::logging::log(crate::logging::Level::Warn, format_args!("{message}")),
        };
        let Some(discovery) = self.discovery.get_async().await else {
            log(format!("Services are unavailable, unable to fetch uri for {service_name}/{endpoint}"));
            return Err(UrlFailure::Offline);
        };
        match discovery["discovery"][service]["endpoints"][endpoint]["uri"].as_str() {
            Some(uri) => Ok(uri.to_string()),
            None => {
                log(format!("Services are available, but unable to find uri for {service_name}/{endpoint}"));
                Err(UrlFailure::MissingEndpoint)
            }
        }
    }

    pub fn profile_key_signature_validator(&self) -> Option<SignatureValidator> {
        let keys = self.key_set.get()?;
        (!keys.profile_keys.is_empty()).then(|| SignatureValidator::Services(keys.profile_keys.clone()))
    }

    pub fn find_profile_by_name(&self, name: &str, callers: Callers) -> Option<NameAndId> {
        let (http, runtime) = self.http.as_ref()?;
        let context: Vec<&'static str> = [FIND_PROFILE_BY_NAME].into_iter().chain(callers.iter().copied()).collect();
        runtime.block_on(async {
            let url = match self.url_async_on("profiles", "getByName", None).await {
                Ok(url) => url,
                Err(failure) => {
                    warn!("Couldn't find profile with name: {name}\n{}", services_unavailable(&failure, &context).throwable.print_stack_trace().trim_end());
                    return None;
                }
            };
            let url = url.replace("{name}", &name.to_lowercase());
            let found = match client_get(http, &url, &context).await {
                Ok(found) => found?,
                Err(exception) => {
                    exception.log_html_body(None);
                    warn!("Couldn't find profile with name: {name}\n{}", exception.throwable.print_stack_trace().trim_end());
                    return None;
                }
            };
            let id = u128::from_str_radix(found["id"].as_str()?, 16).ok()?;
            Some(NameAndId { id, name: found["name"].as_str()?.to_string() })
        })
    }

    pub fn find_profiles_by_names(&self, names: &[&str], callers: Callers, callback: &mut dyn ProfileLookupCallback) -> Result<(), ConversionError> {
        let mut criteria = JavaHashSet::default();
        for name in names.iter().filter(|name| !name.is_empty()) {
            criteria.insert(name.to_string());
        }
        let criteria: Vec<&str> = criteria.iter().collect();
        let not_found_frames: Vec<&'static str> = [FIND_PROFILES_BY_NAMES_NOT_FOUND].into_iter().chain(callers.iter().copied()).collect();
        let failed_frames: Vec<&'static str> = [FIND_PROFILES_BY_NAMES_FAILED].into_iter().chain(callers.iter().copied()).collect();
        for request in criteria.chunks(10) {
            let normalized_request: Vec<String> = request.iter().map(|name| name.to_lowercase()).collect();
            let mut fail_count = 0;
            loop {
                match self.post_many_by_name(&normalized_request, callers) {
                    Ok(results) => {
                        let mut received: Vec<String> = Vec::with_capacity(results.len());
                        for profile in &results {
                            received.push(profile.name.to_lowercase());
                            callback.on_profile_lookup_succeeded(&profile.name, profile.id)?;
                        }
                        for name in request {
                            if !received.contains(&name.to_lowercase()) {
                                let exception = JavaThrowable::new("com.mojang.authlib.services.ProfileNotFoundException: Server did not find the requested profile", not_found_frames.clone());
                                callback.on_profile_lookup_failed(name, &exception, &failed_frames)?;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(100));
                        break;
                    }
                    Err(e) => {
                        e.log_html_body(None);
                        fail_count += 1;
                        if fail_count == 3 {
                            for name in request {
                                let exception = e.to_authentication_exception(&failed_frames);
                                callback.on_profile_lookup_failed(name, &exception, &failed_frames)?;
                            }
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(750));
                    }
                }
            }
        }
        Ok(())
    }

    fn post_many_by_name(&self, names: &[String], callers: Callers) -> Result<Vec<NameAndId>, MinecraftClientException> {
        let context: Vec<&'static str> = [FIND_PROFILES_BY_NAMES_POST].into_iter().chain(callers.iter().copied()).collect();
        let unavailable = |failure: UrlFailure| services_unavailable(&failure, &context);
        let Some(discovery) = self.discovery.get() else {
            warn!("Services are unavailable, unable to fetch uri for PROFILES/getManyByName");
            return Err(unavailable(UrlFailure::Offline));
        };
        let Some(url) = discovery["discovery"]["profiles"]["endpoints"]["getManyByName"]["uri"].as_str() else {
            warn!("Services are available, but unable to find uri for PROFILES/getManyByName");
            return Err(unavailable(UrlFailure::MissingEndpoint));
        };
        let Some((http, runtime)) = self.http.as_ref() else { return Err(unavailable(UrlFailure::Offline)) };
        let body = runtime.block_on(client_post(http, url, names, &context))?;
        let Some(body) = body else { return Ok(Vec::new()) };
        let profiles = body.as_array().cloned().unwrap_or_default();
        profiles
            .iter()
            .map(|profile| {
                let id = u128::from_str_radix(&profile["id"].as_str()?.replace('-', ""), 16).ok()?;
                Some(NameAndId { id, name: profile["name"].as_str()?.to_string() })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| json_failure("[invalid profile]", true, &[READ_SERVICE_RESPONSE_145], &[POST_WITH_ETAG, CLIENT_POST], &context))
    }

    pub fn fetch_profile(&self, id: u128) -> Option<NameAndId> {
        self.fetch_game_profile(id, &[]).map(|profile| NameAndId { id: profile.uuid, name: profile.name })
    }

    pub fn fetch_game_profile(&self, id: u128, callers: Callers) -> Option<crate::network::login::Profile> {
        let (http, runtime) = self.http.as_ref()?;
        let context: Vec<&'static str> = [FETCH_PROFILE].into_iter().chain(callers.iter().copied()).collect();
        runtime.block_on(async {
            let endpoint = match self.url_async_on("session", "getProfileById", None).await {
                Ok(endpoint) => endpoint,
                Err(failure) => {
                    let frames: Vec<&'static str> = [FETCH_PROFILE_UNCACHED_URL].into_iter().chain(context.iter().copied()).collect();
                    warn!(
                        "Couldn't look up profile properties for {}\n{}",
                        crate::world::level::storage::format_uuid(id),
                        services_unavailable(&failure, &frames).throwable.print_stack_trace().trim_end()
                    );
                    return None;
                }
            };
            let endpoint = endpoint.replace("{profileId}", &format!("{id:032x}"));
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            let url = format!("{endpoint}{separator}unsigned=false");
            let frames: Vec<&'static str> = [FETCH_PROFILE_UNCACHED_GET].into_iter().chain(context.iter().copied()).collect();
            let found = match client_get(http, &url, &frames).await {
                Ok(found) => found?,
                Err(exception) => {
                    exception.log_html_body(None);
                    warn!("Couldn't look up profile properties for {}\n{}", crate::world::level::storage::format_uuid(id), exception.throwable.print_stack_trace().trim_end());
                    return None;
                }
            };
            let id = u128::from_str_radix(found["id"].as_str()?, 16).ok()?;
            let properties = found["properties"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
                .iter()
                .filter_map(|property| {
                    Some(crate::network::login::ProfileProperty {
                        name: property["name"].as_str()?.to_string(),
                        value: property["value"].as_str()?.to_string(),
                        signature: property["signature"].as_str().map(str::to_string),
                    })
                })
                .collect();
            Some(crate::network::login::Profile { uuid: id, name: found["name"].as_str()?.to_string(), properties })
        })
    }

    pub fn can_validate_profile_keys(&self) -> bool {
        self.key_set.get().is_some_and(|keys| !keys.profile_keys.is_empty())
    }
}

const DISCOVERY_THREAD: &str = "Minecraft Services Discovery";

const POOL_FRAMES: [&str; 6] = [
    "java.base/java.util.concurrent.Executors$RunnableAdapter.call(Executors.java:545)",
    "java.base/java.util.concurrent.FutureTask.run(FutureTask.java:328)",
    "java.base/java.util.concurrent.ScheduledThreadPoolExecutor$ScheduledFutureTask.run(ScheduledThreadPoolExecutor.java:309)",
    "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1090)",
    "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

const RETRYABLE_FETCH_RUN_40: &str = "com.mojang.authlib.services.RetryableFetch$1.run(RetryableFetch.java:40)";
const RETRYABLE_FETCH_RUN_42: &str = "com.mojang.authlib.services.RetryableFetch$1.run(RetryableFetch.java:42)";

const DISCOVERY_CONTEXT: [&str; 8] = [
    "com.mojang.authlib.services.MinecraftServicesDiscoveryService.lambda$createDiscoverySupplier$1(MinecraftServicesDiscoveryService.java:76)",
    RETRYABLE_FETCH_RUN_42,
    POOL_FRAMES[0],
    POOL_FRAMES[1],
    POOL_FRAMES[2],
    POOL_FRAMES[3],
    POOL_FRAMES[4],
    POOL_FRAMES[5],
];

const KEY_FETCH_CONTEXT: [&str; 9] = [
    "com.mojang.authlib.services.MinecraftServicesKeyInfo.fetch(MinecraftServicesKeyInfo.java:82)",
    "com.mojang.authlib.services.MinecraftServicesKeyInfo.lambda$get$1(MinecraftServicesKeyInfo.java:71)",
    RETRYABLE_FETCH_RUN_42,
    POOL_FRAMES[0],
    POOL_FRAMES[1],
    POOL_FRAMES[2],
    POOL_FRAMES[3],
    POOL_FRAMES[4],
    POOL_FRAMES[5],
];

const KEY_UNAVAILABLE_CONTEXT: [&str; 8] = [
    "com.mojang.authlib.services.MinecraftServicesDiscoveryService.lambda$getServicesKeySet$2(MinecraftServicesDiscoveryService.java:117)",
    RETRYABLE_FETCH_RUN_40,
    POOL_FRAMES[0],
    POOL_FRAMES[1],
    POOL_FRAMES[2],
    POOL_FRAMES[3],
    POOL_FRAMES[4],
    POOL_FRAMES[5],
];

const FIND_PROFILE_BY_NAME: &str = "com.mojang.authlib.services.MinecraftServicesProfileRepository.findProfileByName(MinecraftServicesProfileRepository.java:103)";
const FETCH_PROFILE: &str = "com.mojang.authlib.services.MinecraftServicesSessionService.fetchProfile(MinecraftServicesSessionService.java:161)";
const FETCH_PROFILE_UNCACHED_URL: &str = "com.mojang.authlib.services.MinecraftServicesSessionService.fetchProfileUncached(MinecraftServicesSessionService.java:188)";
const FETCH_PROFILE_UNCACHED_GET: &str = "com.mojang.authlib.services.MinecraftServicesSessionService.fetchProfileUncached(MinecraftServicesSessionService.java:189)";
const FIND_PROFILES_BY_NAMES_POST: &str = "com.mojang.authlib.services.MinecraftServicesProfileRepository.findProfilesByNames(MinecraftServicesProfileRepository.java:55)";
const FIND_PROFILES_BY_NAMES_NOT_FOUND: &str = "com.mojang.authlib.services.MinecraftServicesProfileRepository.findProfilesByNames(MinecraftServicesProfileRepository.java:73)";
const FIND_PROFILES_BY_NAMES_FAILED: &str = "com.mojang.authlib.services.MinecraftServicesProfileRepository.findProfilesByNames(MinecraftServicesProfileRepository.java:86)";

const GET_URL: &str = "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getUrl(MinecraftServicesDiscoveryService.java:125)";
const GET_ENDPOINT_OFFLINE: &str = "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getEndpoint(MinecraftServicesDiscoveryService.java:139)";
const GET_ENDPOINT_MISSING: &str = "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getEndpoint(MinecraftServicesDiscoveryService.java:145)";

const HTTPS_CONNECT_FRAMES: [&str; 17] = [
    "java.base/java.net.SocksSocketImpl.connect(SocksSocketImpl.java:284)",
    "java.base/java.net.Socket.connect(Socket.java:659)",
    "java.base/sun.security.ssl.SSLSocketImpl.connect(SSLSocketImpl.java:304)",
    "java.base/sun.net.NetworkClient.doConnect(NetworkClient.java:161)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:516)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:604)",
    "java.base/sun.net.www.protocol.https.HttpsClient.<init>(HttpsClient.java:206)",
    "java.base/sun.net.www.protocol.https.HttpsClient.New(HttpsClient.java:288)",
    "java.base/sun.net.www.protocol.https.AbstractDelegateHttpsURLConnection.getNewHttpClient(AbstractDelegateHttpsURLConnection.java:196)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect0(HttpURLConnection.java:1008)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect(HttpURLConnection.java:899)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.connect(HttpURLConnection.java:872)",
    "java.base/sun.net.www.protocol.https.AbstractDelegateHttpsURLConnection.connect(AbstractDelegateHttpsURLConnection.java:181)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1377)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
    "java.base/sun.net.www.protocol.https.HttpsURLConnectionImpl.getResponseCode(HttpsURLConnectionImpl.java:307)",
];

const HTTP_CONNECT_FRAMES: [&str; 14] = [
    "java.base/java.net.Socket.connect(Socket.java:659)",
    "java.base/sun.net.NetworkClient.doConnect(NetworkClient.java:161)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:516)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:604)",
    "java.base/sun.net.www.http.HttpClient.<init>(HttpClient.java:276)",
    "java.base/sun.net.www.http.HttpClient.New(HttpClient.java:380)",
    "java.base/sun.net.www.http.HttpClient.New(HttpClient.java:393)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getNewHttpClient(HttpURLConnection.java:1030)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect0(HttpURLConnection.java:1008)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect(HttpURLConnection.java:899)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.connect(HttpURLConnection.java:872)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1377)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
];

const HTTPS_READ_HEADER_FRAMES: [&str; 6] = [
    "java.base/sun.net.www.http.HttpClient.parseHTTP(HttpClient.java:727)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1392)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
    "java.base/sun.net.www.protocol.https.HttpsURLConnectionImpl.getResponseCode(HttpsURLConnectionImpl.java:307)",
    READ_SERVICE_RESPONSE_130,
];

const READ_TIMED_OUT_FRAMES: [&str; 15] = [
    "java.base/sun.nio.ch.NioSocketImpl.timedRead(NioSocketImpl.java:277)",
    "java.base/sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:302)",
    "java.base/sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:354)",
    "java.base/sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:798)",
    "java.base/java.net.Socket$SocketInputStream.implRead(Socket.java:974)",
    "java.base/java.net.Socket$SocketInputStream.read(Socket.java:964)",
    "java.base/sun.security.ssl.SSLSocketInputRecord.read(SSLSocketInputRecord.java:489)",
    "java.base/sun.security.ssl.SSLSocketInputRecord.readHeader(SSLSocketInputRecord.java:483)",
    "java.base/sun.security.ssl.SSLSocketInputRecord.bytesInCompletePacket(SSLSocketInputRecord.java:70)",
    "java.base/sun.security.ssl.SSLSocketImpl.readApplicationRecord(SSLSocketImpl.java:1461)",
    "java.base/sun.security.ssl.SSLSocketImpl$AppInputStream.read(SSLSocketImpl.java:1068)",
    "java.base/java.io.BufferedInputStream.fill(BufferedInputStream.java:289)",
    "java.base/java.io.BufferedInputStream.read1(BufferedInputStream.java:330)",
    "java.base/java.io.BufferedInputStream.read(BufferedInputStream.java:388)",
    "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:795)",
];

const UNEXPECTED_EOF_FRAMES: [&str; 4] = [
    "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:923)",
    "java.base/sun.net.www.http.HttpClient.parseHTTP(HttpClient.java:727)",
    "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:920)",
    "java.base/sun.net.www.http.HttpClient.parseHTTP(HttpClient.java:727)",
];

const PREMATURE_EOF_FRAMES: [&str; 15] = [
    "java.base/sun.net.www.MeteredStream.justRead(MeteredStream.java:58)",
    "java.base/sun.net.www.MeteredStream.read(MeteredStream.java:135)",
    "java.base/java.io.FilterInputStream.read(FilterInputStream.java:119)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection$HttpInputStream.read(HttpURLConnection.java:3322)",
    "java.base/sun.nio.cs.StreamDecoder.readBytes(StreamDecoder.java:279)",
    "java.base/sun.nio.cs.StreamDecoder.implRead(StreamDecoder.java:322)",
    "java.base/sun.nio.cs.StreamDecoder.read(StreamDecoder.java:186)",
    "java.base/java.io.InputStreamReader.read(InputStreamReader.java:183)",
    "java.base/java.io.Reader.read(Reader.java:372)",
    "org.apache.commons.io.IOUtils.copyLarge(IOUtils.java:1579)",
    "org.apache.commons.io.IOUtils.copyLarge(IOUtils.java:1558)",
    "org.apache.commons.io.IOUtils.copy(IOUtils.java:1355)",
    "org.apache.commons.io.IOUtils.copy(IOUtils.java:1128)",
    "org.apache.commons.io.IOUtils.toString(IOUtils.java:3134)",
    READ_SERVICE_RESPONSE_141,
];

const BUILD_HTTP_EXCEPTION_169: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:169)";
const READ_SERVICE_RESPONSE_130: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:130)";
const READ_SERVICE_RESPONSE_141: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:141)";
const READ_SERVICE_RESPONSE_145: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:145)";
const READ_SERVICE_RESPONSE_147: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:147)";
const READ_SERVICE_RESPONSE_151: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:151)";
const GET_WITH_ETAG: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.getWithEtag(MinecraftClient.java:81)";
const CLIENT_GET: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.get(MinecraftClient.java:65)";
const POST_WITH_ETAG: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.postWithEtag(MinecraftClient.java:102)";
const CLIENT_POST: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.post(MinecraftClient.java:94)";
const OBJECT_MAPPER_READ_VALUE_29: &str = "com.mojang.authlib.minecraft.client.ObjectMapper.readValue(ObjectMapper.java:29)";
const OBJECT_MAPPER_READ_VALUE_31: &str = "com.mojang.authlib.minecraft.client.ObjectMapper.readValue(ObjectMapper.java:31)";

const GSON_FROM_JSON: [&str; 4] =
    ["com.google.gson.Gson.fromJson(Gson.java:1080)", "com.google.gson.Gson.fromJson(Gson.java:981)", "com.google.gson.Gson.fromJson(Gson.java:891)", "com.google.gson.Gson.fromJson(Gson.java:828)"];
const REFLECTIVE_ADAPTER_READ_510: &str = "com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:510)";
const REFLECTIVE_ADAPTER_READ_521: &str = "com.google.gson.internal.bind.ReflectiveTypeAdapterFactory$Adapter.read(ReflectiveTypeAdapterFactory.java:521)";

const TREE_TYPE_ADAPTER_FRAMES: [&str; 7] = [
    "com.google.gson.Gson.fromJson(Gson.java:1103)",
    "com.google.gson.Gson.fromJson(Gson.java:1193)",
    "com.google.gson.Gson.fromJson(Gson.java:1164)",
    "com.google.gson.internal.bind.TreeTypeAdapter$GsonContextImpl.deserialize(TreeTypeAdapter.java:206)",
    "com.mojang.authlib.services.response.ProfileSearchResultsResponse$Serializer.deserialize(ProfileSearchResultsResponse.java:18)",
    "com.mojang.authlib.services.response.ProfileSearchResultsResponse$Serializer.deserialize(ProfileSearchResultsResponse.java:15)",
    "com.google.gson.internal.bind.TreeTypeAdapter.read(TreeTypeAdapter.java:95)",
];
const ELEMENT_ADAPTER_FRAMES: [&str; 4] = [
    "com.google.gson.internal.bind.TypeAdapterRuntimeTypeWrapper.read(TypeAdapterRuntimeTypeWrapper.java:40)",
    "com.google.gson.internal.bind.CollectionTypeAdapterFactory$Adapter.read(CollectionTypeAdapterFactory.java:84)",
    "com.google.gson.internal.bind.CollectionTypeAdapterFactory$Adapter.read(CollectionTypeAdapterFactory.java:64)",
    "com.google.gson.Gson.fromJson(Gson.java:1080)",
];
const TREE_TYPE_ADAPTER_FRAMES_AFTER_ELEMENT: [&str; 6] = [
    "com.google.gson.Gson.fromJson(Gson.java:1193)",
    "com.google.gson.Gson.fromJson(Gson.java:1164)",
    TREE_TYPE_ADAPTER_FRAMES[3],
    TREE_TYPE_ADAPTER_FRAMES[4],
    TREE_TYPE_ADAPTER_FRAMES[5],
    TREE_TYPE_ADAPTER_FRAMES[6],
];
const JSON_TREE_READER_BEGIN_OBJECT_FRAMES: [&str; 2] = [
    "com.google.gson.internal.bind.JsonTreeReader.expect(JsonTreeReader.java:184)",
    "com.google.gson.internal.bind.JsonTreeReader.beginObject(JsonTreeReader.java:101)",
];
const JSON_TREE_READER_FRAMES: [&str; 4] = [
    "com.google.gson.internal.bind.JsonTreeReader.expect(JsonTreeReader.java:184)",
    "com.google.gson.internal.bind.JsonTreeReader.beginArray(JsonTreeReader.java:83)",
    "com.google.gson.internal.bind.CollectionTypeAdapterFactory$Adapter.read(CollectionTypeAdapterFactory.java:82)",
    "com.google.gson.internal.bind.CollectionTypeAdapterFactory$Adapter.read(CollectionTypeAdapterFactory.java:64)",
];

enum IoFailure {
    UnknownHost(String),
    ConnectionRefused,
    ConnectTimedOut,
    ReadTimedOut,
    UnexpectedEof,
    PrematureEof,
    Other(String),
}

fn classify_failure(error: &reqwest::Error, url: &str) -> IoFailure {
    let mut chain = String::new();
    let mut source: Option<&dyn std::error::Error> = Some(error);
    let mut refused = false;
    let mut unexpected_eof = false;
    while let Some(current) = source {
        chain.push_str(&current.to_string());
        chain.push(' ');
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            refused |= io.kind() == std::io::ErrorKind::ConnectionRefused;
            unexpected_eof |= io.kind() == std::io::ErrorKind::UnexpectedEof;
        }
        source = current.source();
    }
    let connect_timed_out = error.is_timeout() && chain.contains("client error (Connect)");
    let host = || reqwest::Url::parse(url).ok().and_then(|url| url.host_str().map(str::to_string)).unwrap_or_default();
    let incomplete = chain.contains("connection closed before message completed") || unexpected_eof;
    if chain.contains("dns error") || chain.contains("failed to lookup address") || chain.contains("No such host") {
        IoFailure::UnknownHost(host())
    } else if refused {
        IoFailure::ConnectionRefused
    } else if connect_timed_out {
        IoFailure::ConnectTimedOut
    } else if error.is_timeout() {
        IoFailure::ReadTimedOut
    } else if chain.contains("error reading a body from connection") && incomplete {
        IoFailure::PrematureEof
    } else if incomplete {
        IoFailure::UnexpectedEof
    } else {
        IoFailure::Other(error.to_string())
    }
}

fn connection_refused_message() -> &'static str {
    if cfg!(windows) { "Connection refused: getsockopt" } else { "Connection refused" }
}

fn io_failure(url: &str, failure: IoFailure, tail: &[&'static str]) -> MinecraftClientException {
    let outer_tail: Vec<&'static str> = tail.to_vec();
    let connect_frames: Vec<&'static str> = if url.starts_with("http://") { HTTP_CONNECT_FRAMES.to_vec() } else { HTTPS_CONNECT_FRAMES.to_vec() };
    let (cause_description, cause_head): (String, Vec<&'static str>) = match failure {
        IoFailure::UnknownHost(host) => (
            format!("java.net.UnknownHostException: {host}"),
            ["java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:569)"].into_iter().chain(connect_frames.iter().copied()).chain([READ_SERVICE_RESPONSE_130]).collect(),
        ),
        IoFailure::ConnectionRefused => (
            format!("java.net.ConnectException: {}", connection_refused_message()),
            [
                "java.base/sun.nio.ch.Net.pollConnect(Native Method)",
                "java.base/sun.nio.ch.Net.pollConnectNow(Net.java:639)",
                "java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:543)",
                "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)",
            ]
            .into_iter()
            .chain(connect_frames.iter().copied())
            .chain([READ_SERVICE_RESPONSE_130])
            .collect(),
        ),
        IoFailure::ConnectTimedOut => (
            "java.net.SocketTimeoutException: Connect timed out".to_string(),
            ["java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:547)", "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)"]
                .into_iter()
                .chain(connect_frames.iter().copied())
                .chain([READ_SERVICE_RESPONSE_130])
                .collect(),
        ),
        IoFailure::ReadTimedOut => ("java.net.SocketTimeoutException: Read timed out".to_string(), READ_TIMED_OUT_FRAMES.into_iter().chain(HTTPS_READ_HEADER_FRAMES).collect()),
        IoFailure::UnexpectedEof => ("java.net.SocketException: Unexpected end of file from server".to_string(), UNEXPECTED_EOF_FRAMES.into_iter().chain(HTTPS_READ_HEADER_FRAMES[1..].iter().copied()).collect()),
        IoFailure::PrematureEof => ("java.io.IOException: Premature EOF".to_string(), PREMATURE_EOF_FRAMES.to_vec()),
        IoFailure::Other(message) => (format!("java.io.IOException: {message}"), Vec::new()),
    };
    let message = cause_description.split_once(": ").map_or(cause_description.clone(), |(_, message)| message.to_string());
    let cause_trace: Vec<&'static str> = cause_head.into_iter().chain(outer_tail.iter().copied()).collect();
    let outer_trace: Vec<&'static str> = [READ_SERVICE_RESPONSE_151].into_iter().chain(outer_tail.iter().copied()).collect();
    let message = format!("Failed to read from {url} due to {message}");
    MinecraftClientException {
        error_type: ErrorType::ServiceUnavailable,
        throwable: JavaThrowable::new(format!("com.mojang.authlib.exceptions.MinecraftClientException: {message}"), outer_trace).with_cause(JavaThrowable::new(cause_description, cause_trace)),
        message,
        http: None,
        html_body_log: None,
    }
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "null".to_string(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

fn http_failure(url: &str, status: u16, content_type: Option<&str>, body: &str, call: &[&'static str], context: &[&'static str]) -> MinecraftClientException {
    let tail: Vec<&'static str> = call.iter().copied().chain(context.iter().copied()).collect();
    let frames = |line: &'static str| -> Vec<&'static str> { [line, READ_SERVICE_RESPONSE_147].into_iter().chain(tail.iter().copied()).collect() };
    let mut html_body_log = None;
    let (response, error, message, stack_trace) = if body.is_empty() {
        ("null".to_string(), String::new(), String::new(), frames("com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:170)"))
    } else if content_type.is_some_and(|content_type| content_type.starts_with("text/html")) {
        html_body_log = Some(format!("Got an error with a html body connecting to {url}: {body}"));
        ("null".to_string(), String::new(), String::new(), frames("com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:167)"))
    } else {
        let parsed: serde_json::Value = match serde_json::from_str(body) {
            Ok(parsed @ serde_json::Value::Object(_)) => parsed,
            _ => return json_failure(body, false, &[BUILD_HTTP_EXCEPTION_169, READ_SERVICE_RESPONSE_147], call, context),
        };
        let details = match parsed.get("details") {
            Some(serde_json::Value::Object(map)) => format!("{{{}}}", map.iter().map(|(key, value)| format!("{key}={}", json_text(Some(value)))).collect::<Vec<_>>().join(", ")),
            _ => "null".to_string(),
        };
        let error = parsed.get("error").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
        let error_message = parsed.get("errorMessage").and_then(serde_json::Value::as_str).unwrap_or_default().to_string();
        (
            format!(
                "ErrorResponse[path={}, error={}, errorMessage={}, details={details}]",
                json_text(parsed.get("path")),
                json_text(parsed.get("error")),
                json_text(parsed.get("errorMessage"))
            ),
            error.clone(),
            if !error_message.is_empty() { error_message } else { error },
            frames("com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:170)"),
        )
    };
    let message = if message.is_empty() { format!("Status: {status}") } else { message };
    MinecraftClientException {
        error_type: ErrorType::HttpError,
        message,
        throwable: JavaThrowable::new(format!("MinecraftClientHttpException[type=HTTP_ERROR, status={status}, response={response}]"), stack_trace),
        http: Some((status, error)),
        html_body_log,
    }
}

fn json_failure(body: &str, array_class: bool, site: &[&'static str], call: &[&'static str], context: &[&'static str]) -> MinecraftClientException {
    let tail: Vec<&'static str> = site.iter().copied().chain(call.iter().copied()).chain(context.iter().copied()).collect();
    let message = format!("Failed to read value {body}");
    let outer = JavaThrowable::new(format!("com.mojang.authlib.exceptions.MinecraftClientException: {message}"), [OBJECT_MAPPER_READ_VALUE_31].into_iter().chain(tail.iter().copied()).collect());
    let common: Vec<&'static str> = GSON_FROM_JSON.into_iter().chain([OBJECT_MAPPER_READ_VALUE_29]).chain(tail.iter().copied()).collect();
    let cause = if array_class {
        let parsed = serde_json::from_str::<serde_json::Value>(body);
        let token_of = |value: &serde_json::Value| match value {
            serde_json::Value::Object(_) => "BEGIN_OBJECT",
            serde_json::Value::Array(_) => "BEGIN_ARRAY",
            serde_json::Value::Number(_) => "NUMBER",
            serde_json::Value::Bool(_) => "BOOLEAN",
            _ => "STRING",
        };
        match &parsed {
            Ok(serde_json::Value::Array(elements)) => {
                let (index, element) = elements.iter().enumerate().find(|(_, element)| !element.is_object() && !element.is_null()).expect("a non-object element");
                let description = format!("java.lang.IllegalStateException: Expected BEGIN_OBJECT but was {} at path $[{index}]", token_of(element));
                let exception_frames: Vec<&'static str> = ELEMENT_ADAPTER_FRAMES.into_iter().chain(TREE_TYPE_ADAPTER_FRAMES_AFTER_ELEMENT).chain(common.iter().copied()).collect();
                let state_frames = JSON_TREE_READER_BEGIN_OBJECT_FRAMES.into_iter().chain([REFLECTIVE_ADAPTER_READ_510]).chain(exception_frames.iter().copied()).collect();
                let state = JavaThrowable::new(description.clone(), state_frames);
                JavaThrowable::new(format!("com.google.gson.JsonSyntaxException: {description}"), [REFLECTIVE_ADAPTER_READ_521].into_iter().chain(exception_frames.iter().copied()).collect()).with_cause(state)
            }
            _ => {
                let token = parsed.as_ref().map_or("STRING", token_of);
                let description = format!("java.lang.IllegalStateException: Expected BEGIN_ARRAY but was {token} at path $");
                let state_frames = JSON_TREE_READER_FRAMES.into_iter().chain([GSON_FROM_JSON[0]]).chain(TREE_TYPE_ADAPTER_FRAMES[1..].iter().copied()).chain(common.iter().copied()).collect();
                let state = JavaThrowable::new(description.clone(), state_frames);
                JavaThrowable::new(format!("com.google.gson.JsonSyntaxException: {description}"), TREE_TYPE_ADAPTER_FRAMES.into_iter().chain(common.iter().copied()).collect()).with_cause(state)
            }
        }
    } else {
        let mut reader = JsonReader::new(body);
        reader.strictness = Strictness::Lenient;
        let error = match reader.peek() {
            Ok(JsonToken::BeginObject) | Ok(JsonToken::Null) => None,
            Ok(_) => reader.begin_object().err(),
            Err(error) => Some(error),
        };
        match error {
            Some(error) => {
                let description = format!("java.lang.IllegalStateException: {}", error.message);
                let state_frames = error.stack_trace.into_iter().chain([REFLECTIVE_ADAPTER_READ_510]).chain(common.iter().copied()).collect();
                let state = JavaThrowable::new(description.clone(), state_frames);
                JavaThrowable::new(format!("com.google.gson.JsonSyntaxException: {description}"), [REFLECTIVE_ADAPTER_READ_521].into_iter().chain(common.iter().copied()).collect()).with_cause(state)
            }
            None => JavaThrowable::new("com.google.gson.JsonSyntaxException", common.clone()),
        }
    };
    MinecraftClientException { error_type: ErrorType::JsonError, message, throwable: outer.with_cause(cause), http: None, html_body_log: None }
}

fn services_unavailable(failure: &UrlFailure, context: &[&'static str]) -> MinecraftClientException {
    let line = match failure {
        UrlFailure::Offline => GET_ENDPOINT_OFFLINE,
        UrlFailure::MissingEndpoint => GET_ENDPOINT_MISSING,
    };
    let frames = [line, GET_URL].into_iter().chain(context.iter().copied()).collect();
    MinecraftClientException {
        error_type: ErrorType::ServiceUnavailable,
        message: "Services are unavailable".to_string(),
        throwable: JavaThrowable::new("com.mojang.authlib.exceptions.MinecraftClientException: Services are unavailable", frames),
        http: None,
        html_body_log: None,
    }
}

async fn read_service_response(
    result: Result<reqwest::Response, reqwest::Error>,
    url: &str,
    array_class: bool,
    call: &[&'static str],
    context: &[&'static str],
) -> Result<Option<serde_json::Value>, MinecraftClientException> {
    let tail: Vec<&'static str> = call.iter().copied().chain(context.iter().copied()).collect();
    let response = result.map_err(|e| io_failure(url, classify_failure(&e, url), &tail))?;
    let status = response.status().as_u16();
    if status == 304 {
        return Ok(None);
    }
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_string);
    let body = response.text().await.map_err(|e| io_failure(url, classify_failure(&e, url), &tail))?;
    if status >= 400 {
        return Err(http_failure(url, status, content_type.as_deref(), &body, call, context));
    }
    if body.is_empty() {
        return Ok(None);
    }
    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(serde_json::Value::Null) => Ok(None),
        Ok(value) if (if array_class { value.as_array().is_some_and(|elements| elements.iter().all(|element| element.is_object() || element.is_null())) } else { value.is_object() }) => Ok(Some(value)),
        _ => Err(json_failure(&body, array_class, &[READ_SERVICE_RESPONSE_145], call, context)),
    }
}

async fn client_get(http: &reqwest::Client, url: &str, context: &[&'static str]) -> Result<Option<serde_json::Value>, MinecraftClientException> {
    read_service_response(http.get(url).send().await, url, false, &[GET_WITH_ETAG, CLIENT_GET], context).await
}

async fn client_post(http: &reqwest::Client, url: &str, body: &[String], context: &[&'static str]) -> Result<Option<serde_json::Value>, MinecraftClientException> {
    read_service_response(http.post(url).json(body).send().await, url, true, &[POST_WITH_ETAG, CLIENT_POST], context).await
}

async fn get_json(http: &reqwest::Client, url: &str) -> Option<serde_json::Value> {
    match client_get(http, url, &DISCOVERY_CONTEXT).await {
        Ok(value) => value,
        Err(exception) => {
            exception.log_html_body(Some(DISCOVERY_THREAD));
            warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", exception.throwable.print_stack_trace().trim_end());
            None
        }
    }
}

fn parse_key(base64: &str) -> Option<RsaPublicKey> {
    let der = base64::engine::general_purpose::STANDARD.decode(base64).ok()?;
    RsaPublicKey::from_public_key_der(&der).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CALLER: &str = "Caller.frame(Caller.java:1)";

    #[test]
    fn http_errors_become_authentication_exceptions_with_the_caller_frames() {
        let body = r#"{"path":"/x","error":"NotFound","errorMessage":"Not here"}"#;
        let exception = http_failure("https://example.test/x", 404, Some("application/json"), body, &[CLIENT_GET], &[CALLER]);
        let expected = format!(
            "com.mojang.authlib.exceptions.AuthenticationException: Not here\n\
             \tat com.mojang.authlib.exceptions.MinecraftClientHttpException.toAuthenticationException(MinecraftClientHttpException.java:77)\n\
             \tat {CALLER}\n\
             Caused by: MinecraftClientHttpException[type=HTTP_ERROR, status=404, response=ErrorResponse[path=/x, error=NotFound, errorMessage=Not here, details=null]]\n\
             \tat com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:170)\n\
             \tat {READ_SERVICE_RESPONSE_147}\n\
             \tat {CLIENT_GET}\n\
             \t... 1 more\n"
        );
        assert_eq!(exception.to_authentication_exception(&[CALLER]).print_stack_trace(), expected);
    }

    #[test]
    fn connection_failures_are_unavailable_exceptions_caused_by_the_socket_error() {
        let exception = io_failure("https://example.test/x", IoFailure::ConnectionRefused, &[CALLER]);
        let trace = exception.to_authentication_exception(&[CALLER]).print_stack_trace();
        let refused = connection_refused_message();
        assert!(trace.starts_with(&format!("com.mojang.authlib.exceptions.AuthenticationUnavailableException\n\tat {TO_AUTHENTICATION_EXCEPTION}\n\tat {CALLER}\n")));
        assert!(!trace.contains("Caused by"));
        let trace = exception.throwable.print_stack_trace();
        assert!(trace.starts_with(&format!(
            "com.mojang.authlib.exceptions.MinecraftClientException: Failed to read from https://example.test/x due to {refused}\n\tat {READ_SERVICE_RESPONSE_151}\n\tat {CALLER}\nCaused by: java.net.ConnectException: {refused}\n"
        )));
        assert!(trace.contains("\tat java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)\n"));
        assert!(trace.ends_with(&format!("\tat {READ_SERVICE_RESPONSE_130}\n\t... 1 more\n")));
    }

    #[test]
    fn timeouts_use_the_matching_socket_exception_text() {
        let connect = io_failure("https://example.test/x", IoFailure::ConnectTimedOut, &[CALLER]).throwable.print_stack_trace();
        assert!(connect.contains("Caused by: java.net.SocketTimeoutException: Connect timed out\n\tat java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:547)\n"));
        let read = io_failure("https://example.test/x", IoFailure::ReadTimedOut, &[CALLER]).throwable.print_stack_trace();
        assert!(read.contains("Caused by: java.net.SocketTimeoutException: Read timed out\n"));
        assert!(read.starts_with("com.mojang.authlib.exceptions.MinecraftClientException: Failed to read from https://example.test/x due to Read timed out\n"));
    }

    #[test]
    fn non_object_json_bodies_fail_with_a_gson_syntax_exception() {
        let exception = json_failure("[1]", false, &[READ_SERVICE_RESPONSE_145], &[CLIENT_GET], &[CALLER]);
        let trace = exception.throwable.print_stack_trace();
        assert!(trace.starts_with("com.mojang.authlib.exceptions.MinecraftClientException: Failed to read value [1]\n"));
        assert!(trace.contains("Caused by: com.google.gson.JsonSyntaxException: java.lang.IllegalStateException: Expected BEGIN_OBJECT but was BEGIN_ARRAY at line 1 column 2 path $\n"));
        let authentication = exception.to_authentication_exception(&[CALLER]).print_stack_trace();
        assert!(authentication.starts_with(&format!(
            "com.mojang.authlib.exceptions.AuthenticationException: com.mojang.authlib.exceptions.MinecraftClientException: Failed to read value [1]\n\tat {TO_AUTHENTICATION_EXCEPTION_JSON}\n\tat {CALLER}\nCaused by: "
        )));
    }
}
