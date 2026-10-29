use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use base64::Engine;
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;

use crate::server::players::NameAndId;
use crate::server::players::old_users_converter::{ConversionError, LookupException, ProfileLookupCallback};
use crate::util::java_collections::JavaHashSet;
use crate::util::java_throwable::JavaThrowable;
use crate::util::signature_validator::SignatureValidator;

const DISCOVERY_URL: &str = "https://discovery.minecraftservices.com/minecraft/client";
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_BACKOFF_EXPONENT: u32 = 6;

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

enum MinecraftClientException {
    ServiceUnavailable,
    Http(String),
    Json(String),
}

impl MinecraftClientException {
    fn to_authentication_exception(&self) -> LookupException {
        match self {
            MinecraftClientException::ServiceUnavailable => LookupException::AuthenticationUnavailable,
            MinecraftClientException::Http(message) => LookupException::Authentication(format!("com.mojang.authlib.exceptions.MinecraftClientHttpException: {message}")),
            MinecraftClientException::Json(message) => LookupException::Authentication(format!("com.mojang.authlib.exceptions.MinecraftClientException: {message}")),
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

impl Services {
    pub fn create() -> Services {
        info!("Environment: Environment[discoveryUrl={DISCOVERY_URL}, name=PROD]");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().expect("an HTTP client");
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
                let Some(url) = this.url_async_on("authentication", "getPublicKeys", Some(DISCOVERY_THREAD)).await else {
                    warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", services_unavailable().print_stack_trace().trim_end());
                    this.url_async_on("authentication", "getPublicKeys", Some(DISCOVERY_THREAD)).await;
                    warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", services_unavailable().print_stack_trace().trim_end());
                    return None;
                };
                let response = match client_get(&http, &url, &KEY_FETCH_CONTEXT).await {
                    Ok(response) => response?,
                    Err(throwable) => {
                        for _ in 0..2 {
                            error!(thread: DISCOVERY_THREAD, "Failed to request Minecraft Services public key\n{}", throwable.print_stack_trace().trim_end());
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
        self.url_async_on(service, endpoint, None).await
    }

    async fn url_async_on(&self, service: &str, endpoint: &str, thread: Option<&str>) -> Option<String> {
        let service_name = service.to_uppercase();
        let log = |message: String| match thread {
            Some(thread) => crate::logging::log_as(thread, crate::logging::Level::Warn, format_args!("{message}")),
            None => crate::logging::log(crate::logging::Level::Warn, format_args!("{message}")),
        };
        let Some(discovery) = self.discovery.get_async().await else {
            log(format!("Services are unavailable, unable to fetch uri for {service_name}/{endpoint}"));
            return None;
        };
        let uri = discovery["discovery"][service]["endpoints"][endpoint]["uri"].as_str();
        if uri.is_none() {
            log(format!("Services are available, but unable to find uri for {service_name}/{endpoint}"));
        }
        uri.map(str::to_string)
    }

    pub fn profile_key_signature_validator(&self) -> Option<SignatureValidator> {
        let keys = self.key_set.get()?;
        (!keys.profile_keys.is_empty()).then(|| SignatureValidator::Services(keys.profile_keys.clone()))
    }

    pub fn find_profile_by_name(&self, name: &str) -> Option<NameAndId> {
        let (http, runtime) = self.http.as_ref()?;
        runtime.block_on(async {
            let Some(url) = self.url_async("profiles", "getByName").await else {
                warn!("Couldn't find profile with name: {name}\n{}", services_unavailable_from_endpoint().print_stack_trace().trim_end());
                return None;
            };
            let url = url.replace("{name}", &name.to_lowercase());
            let found = match client_get(http, &url, &[]).await {
                Ok(found) => found?,
                Err(throwable) => {
                    warn!("Couldn't find profile with name: {name}\n{}", throwable.print_stack_trace().trim_end());
                    return None;
                }
            };
            let id = u128::from_str_radix(found["id"].as_str()?, 16).ok()?;
            Some(NameAndId { id, name: found["name"].as_str()?.to_string() })
        })
    }

    pub fn find_profiles_by_names(&self, names: &[&str], callback: &mut dyn ProfileLookupCallback) -> Result<(), ConversionError> {
        let mut criteria = JavaHashSet::default();
        for name in names.iter().filter(|name| !name.is_empty()) {
            criteria.insert(name.to_string());
        }
        let criteria: Vec<&str> = criteria.iter().collect();
        for request in criteria.chunks(10) {
            let normalized_request: Vec<String> = request.iter().map(|name| name.to_lowercase()).collect();
            let mut fail_count = 0;
            loop {
                match self.post_many_by_name(&normalized_request) {
                    Ok(results) => {
                        let mut received: Vec<String> = Vec::with_capacity(results.len());
                        for profile in &results {
                            received.push(profile.name.to_lowercase());
                            callback.on_profile_lookup_succeeded(&profile.name, profile.id)?;
                        }
                        for name in request {
                            if !received.contains(&name.to_lowercase()) {
                                callback.on_profile_lookup_failed(name, &LookupException::ProfileNotFound("Server did not find the requested profile".into()))?;
                            }
                        }
                        std::thread::sleep(Duration::from_millis(100));
                        break;
                    }
                    Err(e) => {
                        fail_count += 1;
                        if fail_count == 3 {
                            for name in request {
                                callback.on_profile_lookup_failed(name, &e.to_authentication_exception())?;
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

    fn post_many_by_name(&self, names: &[String]) -> Result<Vec<NameAndId>, MinecraftClientException> {
        let Some(discovery) = self.discovery.get() else {
            warn!("Services are unavailable, unable to fetch uri for PROFILES/getManyByName");
            return Err(MinecraftClientException::ServiceUnavailable);
        };
        let Some(url) = discovery["discovery"]["profiles"]["endpoints"]["getManyByName"]["uri"].as_str() else {
            warn!("Services are available, but unable to find uri for PROFILES/getManyByName");
            return Err(MinecraftClientException::ServiceUnavailable);
        };
        let Some((http, runtime)) = self.http.as_ref() else { return Err(MinecraftClientException::ServiceUnavailable) };
        let response = runtime
            .block_on(async { http.post(url).json(names).send().await })
            .map_err(|_| MinecraftClientException::ServiceUnavailable)?;
        let status = response.status();
        if !status.is_success() {
            return Err(MinecraftClientException::Http(format!("Status: {}", status.as_u16())));
        }
        let body: serde_json::Value = runtime.block_on(response.json()).map_err(|e| MinecraftClientException::Json(e.to_string()))?;
        let profiles = body.as_array().ok_or_else(|| MinecraftClientException::Json("Expected BEGIN_ARRAY".into()))?;
        profiles
            .iter()
            .map(|profile| {
                let id = u128::from_str_radix(&profile["id"].as_str()?.replace('-', ""), 16).ok()?;
                Some(NameAndId { id, name: profile["name"].as_str()?.to_string() })
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| MinecraftClientException::Json("Invalid profile".into()))
    }

    pub fn fetch_profile(&self, id: u128) -> Option<NameAndId> {
        self.fetch_game_profile(id).map(|profile| NameAndId { id: profile.uuid, name: profile.name })
    }

    pub fn fetch_game_profile(&self, id: u128) -> Option<crate::network::login::Profile> {
        let (http, runtime) = self.http.as_ref()?;
        runtime.block_on(async {
            let Some(endpoint) = self.url_async("session", "getProfileById").await else {
                warn!("Couldn't look up profile properties for {}\n{}", crate::world::level::storage::format_uuid(id), services_unavailable_from_endpoint().print_stack_trace().trim_end());
                return None;
            };
            let endpoint = endpoint.replace("{profileId}", &format!("{id:032x}"));
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            let url = format!("{endpoint}{separator}unsigned=false");
            let found = match client_get(http, &url, &[]).await {
                Ok(found) => found?,
                Err(throwable) => {
                    warn!("Couldn't look up profile properties for {}\n{}", crate::world::level::storage::format_uuid(id), throwable.print_stack_trace().trim_end());
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

const DISCOVERY_CONTEXT: [&str; 2] = [
    "com.mojang.authlib.services.MinecraftServicesDiscoveryService.lambda$createDiscoverySupplier$1(MinecraftServicesDiscoveryService.java:76)",
    "com.mojang.authlib.services.RetryableFetch$1.run(RetryableFetch.java:42)",
];

const KEY_FETCH_CONTEXT: [&str; 0] = [];

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

const READ_SERVICE_RESPONSE_130: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:130)";
const READ_SERVICE_RESPONSE_145: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:145)";
const READ_SERVICE_RESPONSE_147: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:147)";
const READ_SERVICE_RESPONSE_151: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:151)";
const GET_WITH_ETAG: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.getWithEtag(MinecraftClient.java:81)";
const CLIENT_GET: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.get(MinecraftClient.java:65)";

enum IoFailure {
    UnknownHost(String),
    ConnectionRefused,
    ConnectTimedOut,
    ReadTimedOut,
    Other(String),
}

fn classify_failure(error: &reqwest::Error, url: &str) -> IoFailure {
    let mut chain = String::new();
    let mut source: Option<&dyn std::error::Error> = Some(error);
    let mut refused = false;
    while let Some(current) = source {
        chain.push_str(&current.to_string());
        chain.push(' ');
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            refused |= io.kind() == std::io::ErrorKind::ConnectionRefused;
        }
        source = current.source();
    }
    let host = || reqwest::Url::parse(url).ok().and_then(|url| url.host_str().map(str::to_string)).unwrap_or_default();
    if chain.contains("dns error") || chain.contains("failed to lookup address") || chain.contains("No such host") {
        IoFailure::UnknownHost(host())
    } else if refused {
        IoFailure::ConnectionRefused
    } else if error.is_timeout() && error.is_connect() {
        IoFailure::ConnectTimedOut
    } else if error.is_timeout() {
        IoFailure::ReadTimedOut
    } else {
        IoFailure::Other(error.to_string())
    }
}

fn connection_refused_message() -> &'static str {
    if cfg!(windows) { "Connection refused: getsockopt" } else { "Connection refused" }
}

fn io_failure(url: &str, failure: IoFailure, context: &[&'static str]) -> JavaThrowable {
    let outer_tail: Vec<&'static str> = [GET_WITH_ETAG, CLIENT_GET].into_iter().chain(context.iter().copied()).chain(POOL_FRAMES).collect();
    let (cause_description, cause_head): (String, Vec<&'static str>) = match failure {
        IoFailure::UnknownHost(host) => (format!("java.net.UnknownHostException: {host}"), [
            "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:569)",
        ].into_iter().chain(HTTPS_CONNECT_FRAMES).collect()),
        IoFailure::ConnectionRefused => (format!("java.net.ConnectException: {}", connection_refused_message()), [
            "java.base/sun.nio.ch.Net.pollConnect(Native Method)",
            "java.base/sun.nio.ch.Net.pollConnectNow(Net.java:639)",
            "java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:543)",
            "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)",
        ].into_iter().chain(HTTPS_CONNECT_FRAMES).collect()),
        IoFailure::ConnectTimedOut => ("java.net.SocketTimeoutException: Connect timed out".to_string(), Vec::new()),
        IoFailure::ReadTimedOut => ("java.net.SocketTimeoutException: Read timed out".to_string(), Vec::new()),
        IoFailure::Other(message) => (format!("java.io.IOException: {message}"), Vec::new()),
    };
    let message = cause_description.split_once(": ").map_or(cause_description.clone(), |(_, message)| message.to_string());
    let cause_trace: Vec<&'static str> = cause_head.into_iter().chain([READ_SERVICE_RESPONSE_130]).chain(outer_tail.iter().copied()).collect();
    let outer_trace: Vec<&'static str> = [READ_SERVICE_RESPONSE_151].into_iter().chain(outer_tail.iter().copied()).collect();
    JavaThrowable::new(format!("com.mojang.authlib.exceptions.MinecraftClientException: Failed to read from {url} due to {message}"), outer_trace).with_cause(JavaThrowable::new(cause_description, cause_trace))
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    match value {
        None | Some(serde_json::Value::Null) => "null".to_string(),
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(other) => other.to_string(),
    }
}

fn http_failure(url: &str, status: u16, content_type: Option<&str>, body: &str, context: &[&'static str]) -> JavaThrowable {
    let tail: Vec<&'static str> = [READ_SERVICE_RESPONSE_147, GET_WITH_ETAG, CLIENT_GET].into_iter().chain(context.iter().copied()).chain(POOL_FRAMES).collect();
    let (response, line) = if body.is_empty() {
        ("null".to_string(), "com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:160)")
    } else if content_type.is_some_and(|content_type| content_type.starts_with("text/html")) {
        error!("Got an error with a html body connecting to {url}: {body}");
        ("null".to_string(), "com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:167)")
    } else {
        let parsed: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
        let details = match parsed.get("details") {
            Some(serde_json::Value::Object(map)) => format!("{{{}}}", map.iter().map(|(key, value)| format!("{key}={}", json_text(Some(value)))).collect::<Vec<_>>().join(", ")),
            _ => "null".to_string(),
        };
        (
            format!(
                "ErrorResponse[path={}, error={}, errorMessage={}, details={details}]",
                json_text(parsed.get("path")),
                json_text(parsed.get("error")),
                json_text(parsed.get("errorMessage"))
            ),
            "com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:170)",
        )
    };
    JavaThrowable::new(format!("MinecraftClientHttpException[type=HTTP_ERROR, status={status}, response={response}]"), [line].into_iter().chain(tail).collect())
}

async fn client_get(http: &reqwest::Client, url: &str, context: &[&'static str]) -> Result<Option<serde_json::Value>, JavaThrowable> {
    let response = http.get(url).send().await.map_err(|e| io_failure(url, classify_failure(&e, url), context))?;
    let status = response.status().as_u16();
    if status == 304 {
        return Ok(None);
    }
    let content_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|value| value.to_str().ok()).map(str::to_string);
    let body = response.text().await.map_err(|e| io_failure(url, classify_failure(&e, url), context))?;
    if status >= 400 {
        return Err(http_failure(url, status, content_type.as_deref(), &body, context));
    }
    if body.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&body).map(Some).map_err(|_| {
        let tail: Vec<&'static str> = [READ_SERVICE_RESPONSE_145, GET_WITH_ETAG, CLIENT_GET].into_iter().chain(context.iter().copied()).chain(POOL_FRAMES).collect();
        JavaThrowable::new(format!("com.mojang.authlib.exceptions.MinecraftClientException: Failed to read value {body}"), ["com.mojang.authlib.minecraft.client.ObjectMapper.readValue(ObjectMapper.java:31)"].into_iter().chain(tail).collect())
    })
}

async fn get_json(http: &reqwest::Client, url: &str) -> Option<serde_json::Value> {
    match client_get(http, url, &DISCOVERY_CONTEXT).await {
        Ok(value) => value,
        Err(throwable) => {
            warn!(thread: DISCOVERY_THREAD, "Failed to connect\n{}", throwable.print_stack_trace().trim_end());
            None
        }
    }
}

fn services_unavailable_from_endpoint() -> JavaThrowable {
    let frames = [
        "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getEndpoint(MinecraftServicesDiscoveryService.java:139)",
        "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getUrl(MinecraftServicesDiscoveryService.java:125)",
    ];
    JavaThrowable::new("com.mojang.authlib.exceptions.MinecraftClientException: Services are unavailable", frames.to_vec())
}

fn services_unavailable() -> JavaThrowable {
    let frames = [
        "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getEndpoint(MinecraftServicesDiscoveryService.java:139)",
        "com.mojang.authlib.services.MinecraftServicesDiscoveryService.getUrl(MinecraftServicesDiscoveryService.java:125)",
        "com.mojang.authlib.services.MinecraftServicesDiscoveryService.lambda$getServicesKeySet$2(MinecraftServicesDiscoveryService.java:117)",
        "com.mojang.authlib.services.RetryableFetch$1.run(RetryableFetch.java:40)",
    ];
    JavaThrowable::new("com.mojang.authlib.exceptions.MinecraftClientException: Services are unavailable", frames.into_iter().chain(POOL_FRAMES).collect())
}

fn parse_key(base64: &str) -> Option<RsaPublicKey> {
    let der = base64::engine::general_purpose::STANDARD.decode(base64).ok()?;
    RsaPublicKey::from_public_key_der(&der).ok()
}
