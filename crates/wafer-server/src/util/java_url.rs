use crate::util::java_throwable::JavaThrowable;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaUrl {
    pub protocol: String,
    pub authority: Option<String>,
    pub host: String,
    pub port: i32,
    pub path: String,
    pub query: Option<String>,
    pub reference: Option<String>,
}

const KNOWN_PROTOCOLS: [&str; 7] = ["http", "https", "file", "jar", "jrt", "ftp", "mailto"];

const CONSTANT_URL_ERROR: &str = "com.mojang.authlib.HttpDiscoveryService.constantURL(HttpDiscoveryService.java:41)";
const CONSTANT_URL_NEW_URL: &str = "com.mojang.authlib.HttpDiscoveryService.constantURL(HttpDiscoveryService.java:39)";
const URL_CONSTRUCTOR_TAIL: [&str; 2] = ["java.base/java.net.URL.<init>(URL.java:630)", "java.base/java.net.URL.<init>(URL.java:566)"];

fn malformed_at(message: impl Into<String>, line: u32, cause: Option<&str>) -> JavaThrowable {
    let message = message.into();
    let head: &'static str = Box::leak(format!("java.base/java.net.URL.<init>(URL.java:{line})").into_boxed_str());
    let mut frames = vec![head];
    frames.extend(URL_CONSTRUCTOR_TAIL);
    let throwable = JavaThrowable::new(format!("java.net.MalformedURLException: {message}"), frames);
    match cause {
        Some(class) => throwable.with_cause(JavaThrowable::new(format!("{class}: {message}"), Vec::new())),
        None => throwable,
    }
}

fn malformed(message: impl Into<String>) -> JavaThrowable {
    malformed_at(message, 769, Some("java.lang.IllegalArgumentException"))
}

fn is_illegal_host_character(c: char) -> bool {
    c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`') || c == '\u{7f}'
}

impl JavaUrl {
    pub fn parse(spec: &str) -> Result<JavaUrl, JavaThrowable> {
        let trimmed = spec.trim_matches(|c: char| c <= ' ');
        let (protocol, rest) = split_protocol(trimmed).ok_or_else(|| malformed_at(format!("no protocol: {spec}"), 735, None))?;
        if !KNOWN_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(malformed_at(format!("unknown protocol: {protocol}"), 742, None));
        }
        let (rest, reference) = match rest.split_once('#') {
            Some((rest, reference)) => (rest, Some(reference.to_string())),
            None => (rest, None),
        };
        let (authority, file) = match rest.strip_prefix("//") {
            Some(after) => {
                let end = after.find(['/', '?']).unwrap_or(after.len());
                (Some(after[..end].to_string()), &after[end..])
            }
            None => (None, rest),
        };
        let (host, port) = match &authority {
            Some(authority) => parse_authority(authority)?,
            None => (String::new(), -1),
        };
        let (path, query) = match file.split_once('?') {
            Some((path, query)) => (path.to_string(), Some(query.to_string())),
            None => (file.to_string(), None),
        };
        Ok(JavaUrl { protocol, authority, host, port, path, query, reference })
    }

    pub fn constant(spec: &str) -> Result<JavaUrl, JavaThrowable> {
        JavaUrl::parse(spec).map_err(|cause| {
            let mut cause = cause;
            cause.push_frame(CONSTANT_URL_NEW_URL);
            JavaThrowable::new(format!("java.lang.Error: Couldn't create constant for {spec}"), vec![CONSTANT_URL_ERROR]).with_cause(cause)
        })
    }

    pub fn is_https(&self) -> bool {
        self.protocol == "https"
    }

    pub fn default_port(&self) -> u16 {
        if self.is_https() { 443 } else { 80 }
    }

    pub fn effective_port(&self) -> u16 {
        if self.port < 0 { self.default_port() } else { self.port as u16 }
    }

    pub fn file(&self) -> String {
        match &self.query {
            Some(query) => format!("{}?{query}", self.path),
            None => self.path.clone(),
        }
    }

    pub fn request_target(&self) -> String {
        let file = self.file();
        if file.is_empty() { "/".to_string() } else if file.starts_with('?') { format!("/{file}") } else { file }
    }

    pub fn host_header(&self) -> String {
        let host = if self.host.contains(':') && !self.host.starts_with('[') { format!("[{}]", self.host) } else { self.host.clone() };
        if self.port < 0 || self.port as u16 == self.default_port() { host } else { format!("{host}:{}", self.port) }
    }

    pub fn with_query(&self, query: &str) -> JavaUrl {
        let mut url = self.clone();
        url.query = Some(match &self.query {
            Some(existing) if !existing.is_empty() => format!("{existing}&{query}"),
            _ => query.to_string(),
        });
        url.reference = None;
        url
    }
}

impl std::fmt::Display for JavaUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:", self.protocol)?;
        if let Some(authority) = &self.authority {
            write!(formatter, "//{authority}")?;
        }
        write!(formatter, "{}", self.file())?;
        if let Some(reference) = &self.reference {
            write!(formatter, "#{reference}")?;
        }
        Ok(())
    }
}

fn split_protocol(spec: &str) -> Option<(String, &str)> {
    let mut chars = spec.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    for (index, c) in chars {
        if c == ':' {
            return Some((spec[..index].to_ascii_lowercase(), &spec[index + 1..]));
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
            return None;
        }
    }
    None
}

fn parse_authority(authority: &str) -> Result<(String, i32), JavaThrowable> {
    let host_and_port = authority.rsplit_once('@').map_or(authority, |(_, after)| after);
    let (host, port_text) = if host_and_port.starts_with('[') {
        match host_and_port.find(']') {
            Some(end) => {
                let after = &host_and_port[end + 1..];
                match after.strip_prefix(':') {
                    Some(port) => (&host_and_port[..=end], Some(port)),
                    None if after.is_empty() => (&host_and_port[..=end], None),
                    None => return Err(malformed(format!("Invalid authority field: {authority}"))),
                }
            }
            None => return Err(malformed(format!("Invalid authority field: {host_and_port}"))),
        }
    } else {
        match host_and_port.rfind(':') {
            Some(index) => (&host_and_port[..index], Some(&host_and_port[index + 1..])),
            None => (host_and_port, None),
        }
    };
    if let Some(c) = host.chars().find(|c| is_illegal_host_character(*c)) {
        return Err(malformed(format!("Illegal character found in host: '{c}'")));
    }
    let port = match port_text {
        Some("") | None => -1,
        Some(text) => match text.parse::<i32>() {
            Ok(port) if port >= -1 => port,
            Ok(port) => return Err(malformed(format!("Invalid port number :{port}"))),
            Err(_) => return Err(malformed(format!("Error at index 0 in: \"{text}\""))),
        },
    };
    Ok((host.to_string(), port))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(spec: &str) -> String {
        JavaUrl::parse(spec).unwrap_err().description
    }

    #[test]
    fn malformed_urls_use_the_jdk_messages() {
        assert_eq!(message("notaurl"), "java.net.MalformedURLException: no protocol: notaurl");
        assert_eq!(message(""), "java.net.MalformedURLException: no protocol: ");
        assert_eq!(message("foo://x"), "java.net.MalformedURLException: unknown protocol: foo");
        assert_eq!(message("://x"), "java.net.MalformedURLException: no protocol: ://x");
        assert_eq!(message("1http://x"), "java.net.MalformedURLException: no protocol: 1http://x");
        assert_eq!(message("http://host:abc/"), "java.net.MalformedURLException: Error at index 0 in: \"abc\"");
        assert_eq!(message("http://host:-5/"), "java.net.MalformedURLException: Invalid port number :-5");
        assert_eq!(message("http://[::1/"), "java.net.MalformedURLException: Invalid authority field: [::1");
        assert_eq!(message("https://exa mple.com/x"), "java.net.MalformedURLException: Illegal character found in host: ' '");
    }

    #[test]
    fn accepted_urls_split_like_the_jdk() {
        let url = JavaUrl::parse("HTTPS://127.0.0.1:26001/d?x=1#f").unwrap();
        assert_eq!((url.protocol.as_str(), url.host.as_str(), url.port, url.file().as_str(), url.reference.as_deref()), ("https", "127.0.0.1", 26001, "/d?x=1", Some("f")));
        assert_eq!(url.to_string(), "https://127.0.0.1:26001/d?x=1#f");
        assert_eq!(JavaUrl::parse("HTTP://X/").unwrap().to_string(), "http://X/");
        assert_eq!(JavaUrl::parse("http://").unwrap().host, "");
        assert_eq!(JavaUrl::parse("http://host:/p").unwrap().port, -1);
        assert_eq!(JavaUrl::parse("http://user@host/").unwrap().host, "host");
    }

    #[test]
    fn constant_urls_throw_an_error() {
        let error = JavaUrl::constant("notaurl").unwrap_err();
        assert_eq!(error.description, "java.lang.Error: Couldn't create constant for notaurl");
        assert_eq!(error.cause.as_ref().unwrap().description, "java.net.MalformedURLException: no protocol: notaurl");
    }
}
