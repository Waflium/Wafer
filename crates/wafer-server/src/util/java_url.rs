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

fn malformed_at(message: impl Into<String>, head: &'static str) -> JavaThrowable {
    let mut frames = vec![head];
    frames.extend(URL_CONSTRUCTOR_TAIL);
    JavaThrowable::new(format!("java.net.MalformedURLException: {}", message.into()), frames)
}

fn malformed(message: impl Into<String>, cause_class: &str, cause_frames: &[&'static str]) -> JavaThrowable {
    let message = message.into();
    let mut frames = vec!["java.base/java.net.URL.<init>(URL.java:769)"];
    frames.extend(URL_CONSTRUCTOR_TAIL);
    let mut cause = cause_frames.to_vec();
    cause.push("java.base/java.net.URL.<init>(URL.java:764)");
    cause.extend(URL_CONSTRUCTOR_TAIL);
    JavaThrowable::new(format!("java.net.MalformedURLException: {message}"), frames).with_cause(JavaThrowable::new(format!("{cause_class}: {message}"), cause))
}

fn illegal_argument(message: impl Into<String>, parse_url_line: &'static str) -> JavaThrowable {
    malformed(message, "java.lang.IllegalArgumentException", &[parse_url_line])
}

fn parse_port(text: &str, parse_url_line: &'static str) -> Result<i32, JavaThrowable> {
    java_parse_int(text).map_err(|error_index| {
        malformed(
            format!("Error at index {error_index} in: \"{text}\""),
            "java.lang.NumberFormatException",
            &["java.base/java.lang.NumberFormatException.forCharSequence(NumberFormatException.java:83)", "java.base/java.lang.Integer.parseInt(Integer.java:640)", parse_url_line],
        )
    })
}

fn java_parse_int(text: &str) -> Result<i32, usize> {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut negative = false;
    let mut limit = -i32::MAX;
    if let Some(&first) = chars.first()
        && first < '0'
    {
        match first {
            '-' => {
                negative = true;
                limit = i32::MIN;
            }
            '+' => {}
            _ => return Err(0),
        }
        i = 1;
        if i == chars.len() {
            return Err(i);
        }
    }
    let multmin = limit / 10;
    let mut result: i32 = 0;
    while i < chars.len() {
        let Some(digit) = chars[i].to_digit(10).map(|digit| digit as i32) else { return Err(i) };
        if result < multmin {
            return Err(i);
        }
        result *= 10;
        if result < limit + digit {
            return Err(i);
        }
        i += 1;
        result -= digit;
    }
    Ok(if negative { result } else { -result })
}

fn is_illegal_host_character(c: char) -> bool {
    c <= ' ' || matches!(c, '<' | '>' | '"' | '{' | '}' | '|' | '\\' | '^' | '`') || c == '\u{7f}'
}

impl JavaUrl {
    pub fn parse(spec: &str) -> Result<JavaUrl, JavaThrowable> {
        let trimmed = spec.trim_matches(|c: char| c <= ' ');
        let (protocol, rest) = split_protocol(trimmed).ok_or_else(|| malformed_at(format!("no protocol: {spec}"), "java.base/java.net.URL.<init>(URL.java:735)"))?;
        if !KNOWN_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(malformed_at(format!("unknown protocol: {protocol}"), "java.base/java.net.URL.<init>(URL.java:742)"));
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

    pub fn resolve(&self, spec: &str) -> JavaUrl {
        let spec = spec.trim_matches(|c: char| c <= ' ');
        let (spec, reference) = match spec.split_once('#') {
            Some((spec, reference)) => (spec, Some(reference.to_string())),
            None => (spec, None),
        };
        let mut url = JavaUrl { query: None, reference, ..self.clone() };
        if spec.is_empty() {
            url.query = self.query.clone();
            if url.reference.is_none() {
                url.reference = self.reference.clone();
            }
            return url;
        }
        let query_only = spec.starts_with('?');
        let spec = match spec.split_once('?') {
            Some((spec, query)) => {
                url.query = Some(query.to_string());
                spec
            }
            None => spec,
        };
        let mut rest = spec;
        if !rest.starts_with("////") && rest.starts_with("//") {
            let after = &rest[2..];
            let end = after.find('/').unwrap_or(after.len());
            let authority = &after[..end];
            let (host, port) = parse_authority(authority).unwrap_or_else(|_| (authority.to_string(), -1));
            url.authority = Some(authority.to_string());
            url.host = host;
            url.port = port;
            if !authority.is_empty() {
                url.path = String::new();
            }
            rest = &after[end..];
        }
        let mut relative = false;
        if !rest.is_empty() {
            if rest.starts_with('/') {
                url.path = rest.to_string();
            } else if !url.path.is_empty() {
                relative = true;
                url.path = match url.path.rfind('/') {
                    None if url.authority.is_some() => format!("/{rest}"),
                    index => format!("{}{rest}", &url.path[..index.map_or(0, |index| index + 1)]),
                };
            } else {
                url.path = if url.authority.is_some() { format!("/{rest}") } else { rest.to_string() };
            }
        } else if query_only {
            url.path = match url.path.rfind('/') {
                Some(index) => url.path[..index + 1].to_string(),
                None => "/".to_string(),
            };
        }
        if relative {
            url.path = remove_dot_segments(url.path);
        }
        url
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

fn remove_dot_segments(mut path: String) -> String {
    while let Some(index) = path.find("/./") {
        path.replace_range(index..index + 2, "");
    }
    let mut from = 0;
    while let Some(found) = path[from..].find("/../") {
        let index = from + found;
        match path[..index].rfind('/').filter(|&previous| index > 0 && !(previous == 0 && path.starts_with("/../"))) {
            Some(previous) => {
                path.replace_range(previous..index + 3, "");
                from = 0;
            }
            None => from = index + 3,
        }
    }
    while path.ends_with("/..") {
        let index = path.find("/..").expect("a /.. suffix");
        match index.checked_sub(1).and_then(|end| path[..=end].rfind('/')) {
            Some(previous) => path.truncate(previous + 1),
            None => break,
        }
    }
    if path.starts_with("./") && path.len() > 2 {
        path.replace_range(..2, "");
    }
    if path.ends_with("/.") {
        path.pop();
    }
    path
}

fn parse_authority(authority: &str) -> Result<(String, i32), JavaThrowable> {
    let host_and_port = authority.rsplit_once('@').map_or(authority, |(_, after)| after);
    let bracketed = host_and_port.starts_with('[');
    let (host, port_text) = if bracketed {
        match host_and_port.find(']') {
            Some(end) => {
                let after = &host_and_port[end + 1..];
                match after.strip_prefix(':') {
                    Some(port) => (&host_and_port[..=end], Some(port)),
                    None if after.is_empty() => (&host_and_port[..=end], None),
                    None => return Err(illegal_argument(format!("Invalid authority field: {authority}"), "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:226)")),
                }
            }
            None => return Err(illegal_argument(format!("Invalid authority field: {authority}"), "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:232)")),
        }
    } else {
        match host_and_port.rfind(':') {
            Some(index) => (&host_and_port[..index], Some(&host_and_port[index + 1..])),
            None => (host_and_port, None),
        }
    };
    let port = match port_text {
        Some("") | None => -1,
        Some(text) => {
            let line = if bracketed { "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:221)" } else { "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:241)" };
            match parse_port(text, line)? {
                port if port >= -1 => port,
                port => return Err(illegal_argument(format!("Invalid port number :{port}"), "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:251)")),
            }
        }
    };
    if let Some(c) = host.chars().find(|c| is_illegal_host_character(*c)) {
        return Err(malformed(
            format!("Illegal character found in host: '{c}'"),
            "java.lang.IllegalArgumentException",
            &["java.base/java.net.URLStreamHandler.setURL(URLStreamHandler.java:528)", "java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:331)"],
        ));
    }
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
    fn malformed_url_causes_carry_the_jdk_frames() {
        let trace = |spec: &str| {
            let mut error = JavaUrl::parse(spec).unwrap_err();
            error.push_frame("Caller.main(Caller.java:7)");
            error.print_stack_trace()
        };
        assert_eq!(
            trace("http://host:99999999999/"),
            "java.net.MalformedURLException: Error at index 9 in: \"99999999999\"\n\
             \tat java.base/java.net.URL.<init>(URL.java:769)\n\
             \tat java.base/java.net.URL.<init>(URL.java:630)\n\
             \tat java.base/java.net.URL.<init>(URL.java:566)\n\
             \tat Caller.main(Caller.java:7)\n\
             Caused by: java.lang.NumberFormatException: Error at index 9 in: \"99999999999\"\n\
             \tat java.base/java.lang.NumberFormatException.forCharSequence(NumberFormatException.java:83)\n\
             \tat java.base/java.lang.Integer.parseInt(Integer.java:640)\n\
             \tat java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:241)\n\
             \tat java.base/java.net.URL.<init>(URL.java:764)\n\
             \t... 3 more\n"
        );
        assert!(trace("http://h:1a/").contains("Error at index 1 in: \"1a\""));
        assert!(trace("http://h:+/").contains("Error at index 1 in: \"+\""));
        assert!(trace("http://h:2147483648/").contains("Error at index 9 in: \"2147483648\""));
        assert!(trace("http://h:-2147483649/").contains("Error at index 10 in: \"-2147483649\""));
        assert!(trace("http://[::1]:ab/").contains("URLStreamHandler.parseURL(URLStreamHandler.java:221)"));
        assert!(trace("http://[::1]x/").contains("Caused by: java.lang.IllegalArgumentException: Invalid authority field: [::1]x\n\tat java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:226)"));
        assert!(trace("http://u@[::1/").contains("Caused by: java.lang.IllegalArgumentException: Invalid authority field: u@[::1\n\tat java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:232)"));
        assert!(trace("http://h:-5/").contains("Caused by: java.lang.IllegalArgumentException: Invalid port number :-5\n\tat java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:251)"));
        assert!(trace("http://a<b/").contains(
            "Caused by: java.lang.IllegalArgumentException: Illegal character found in host: '<'\n\tat java.base/java.net.URLStreamHandler.setURL(URLStreamHandler.java:528)\n\tat java.base/java.net.URLStreamHandler.parseURL(URLStreamHandler.java:331)"
        ));
        assert_eq!(JavaUrl::parse("http://h:+5/").unwrap().port, 5);
        assert_eq!(JavaUrl::parse("http://h:-1/").unwrap().port, -1);
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

    #[test]
    fn relative_specs_resolve_like_url_with_a_context() {
        let cases = [
            ("http://h/a/b/c?q#f", "d", "http://h/a/b/d"),
            ("http://h/a/b/c?q#f", "./d", "http://h/a/b/d"),
            ("http://h/a/b/c?q#f", "../d", "http://h/a/d"),
            ("http://h/a/b/c?q#f", "../../d", "http://h/d"),
            ("http://h/a/b/c?q#f", "../../../d", "http://h/../d"),
            ("http://h/a/b/c?q#f", "/x/y", "http://h/x/y"),
            ("http://h/a/b/c?q#f", "?z", "http://h/a/b/?z"),
            ("http://h/a/b/c?q#f", "#g", "http://h/a/b/c?q#g"),
            ("http://h/a/b/c?q#f", "", "http://h/a/b/c?q#f"),
            ("http://h/a/b/c?q#f", "//other:8080/p?w", "http://other:8080/p?w"),
            ("http://h/a/b/c?q#f", "//other", "http://other"),
            ("http://h/a/b/", "..", "http://h/a/"),
            ("http://h/a/b/", ".", "http://h/a/b/"),
            ("http://h/a/b/", "./", "http://h/a/b/"),
            ("http://h/a/b/c", "d/../e", "http://h/a/b/e"),
            ("http://h/a/b/c", "d/./e/.", "http://h/a/b/d/e/"),
            ("http://h", "d", "http://h/d"),
            ("http://h:81/a", "?x#y", "http://h:81/?x#y"),
            ("https://h/a/b/c", "g;x?y#s", "https://h/a/b/g;x?y#s"),
            ("http://h/a/b/c", "////x", "http://h////x"),
            ("http://h/a/b/c", "..//x", "http://h/a//x"),
            ("http://h/a/b/c", "  spaced  ", "http://h/a/b/spaced"),
        ];
        for (base, spec, expected) in cases {
            assert_eq!(JavaUrl::parse(base).unwrap().resolve(spec).to_string(), expected, "{base} + {spec:?}");
        }
    }
}
