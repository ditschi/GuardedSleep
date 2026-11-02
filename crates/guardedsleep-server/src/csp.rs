//! Content-Security-Policy header of the static web UI (`--serve-web`).
//!
//! The policy is the one `apps/web/csp/csp.ts` puts into index.html as a `<meta>` fallback; the
//! Vitest `csp.test.ts` compares both (and the nginx header) so they cannot drift. Two policies on
//! one page must both allow a request, so the header has to allow exactly the WebSocket origins
//! the page was built with: the web build writes them to [`CONNECT_ORIGINS_FILE`] next to
//! `index.html`, and the server reads that file when it starts.

use std::io;
use std::path::Path;

/// Written by the web build (`apps/web/csp/csp.ts`) into the served directory.
pub const CONNECT_ORIGINS_FILE: &str = "csp-connect-origins.json";

const BEFORE_CONNECT: &str =
    "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'";
const AFTER_CONNECT: &str = "img-src 'self' data: blob:; object-src 'none'; base-uri 'none'; \
     frame-ancestors 'none'; require-trusted-types-for 'script'; \
     trusted-types guardedsleep svelte-trusted-html";

/// `wss://host[:port]` or `ws://host[:port]` and nothing else: the value ends up in a header.
pub fn valid_connect_origin(origin: &str) -> bool {
    let Some(rest) = origin
        .strip_prefix("wss://")
        .or_else(|| origin.strip_prefix("ws://"))
    else {
        return false;
    };
    let (host, port) = match rest.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (rest, None),
    };
    !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        && port.is_none_or(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// The header value: the page itself plus `extra_connect` as WebSocket origins.
pub fn header_value(extra_connect: &[String]) -> String {
    let mut connect = String::new();
    for origin in extra_connect {
        connect.push(' ');
        connect.push_str(origin);
    }
    format!("{BEFORE_CONNECT}{connect}; {AFTER_CONNECT}")
}

/// The origins the web build allowed, from `<dir>/csp-connect-origins.json`. A missing file means
/// none (a hand-made directory); a file that cannot be read as `{"origins":["wss://…"]}` is an
/// error, so a typo never silently widens or narrows the policy.
pub fn load_connect_origins(dir: &Path) -> io::Result<Vec<String>> {
    let path = dir.join(CONNECT_ORIGINS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let invalid = |why: String| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {why}", path.display()),
        )
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?;
    let list = value
        .get("origins")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| invalid("expected {\"origins\": [...]}".into()))?;
    list.iter()
        .map(|item| match item.as_str() {
            Some(origin) if valid_connect_origin(origin) => Ok(origin.to_owned()),
            _ => Err(invalid(format!("not a ws:// or wss:// origin: {item}"))),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_self_only() {
        let value = header_value(&[]);
        assert!(value.starts_with("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; "));
        assert!(value.contains("connect-src 'self'; img-src"));
        assert!(value.contains("require-trusted-types-for 'script'"));
        assert!(value.contains("trusted-types guardedsleep svelte-trusted-html"));
        assert!(value.contains("frame-ancestors 'none'"));
        assert!(!value.contains("unsafe-inline"));
        assert!(!value.contains("unsafe-eval'") || value.contains("wasm-unsafe-eval"));
    }

    #[test]
    fn extra_origins_extend_connect_src_only() {
        let value = header_value(&["wss://a.example".into(), "ws://b.example:9".into()]);
        assert!(value.contains("connect-src 'self' wss://a.example ws://b.example:9; img-src"));
    }

    #[test]
    fn origins_are_checked() {
        for ok in [
            "wss://relay.example.org",
            "ws://192.168.1.2:8787",
            "wss://a-b.c:1",
        ] {
            assert!(valid_connect_origin(ok), "{ok}");
        }
        for bad in [
            "",
            "https://a.example",
            "wss://",
            "wss://a.example/path",
            "wss://a.example; script-src *",
            "wss://a.example:",
            "wss://a.example:123456",
            "wss://a b",
            "wss://a.example:80a",
        ] {
            assert!(!valid_connect_origin(bad), "{bad}");
        }
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("gs-csp-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn loads_the_origins_of_the_web_build() {
        let d = dir("ok");
        assert_eq!(load_connect_origins(&d).unwrap(), Vec::<String>::new());
        std::fs::write(
            d.join(CONNECT_ORIGINS_FILE),
            r#"{"origins":["wss://relay.example.org"]}"#,
        )
        .unwrap();
        assert_eq!(
            load_connect_origins(&d).unwrap(),
            vec!["wss://relay.example.org".to_owned()]
        );
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn refuses_a_broken_origins_file() {
        let d = dir("bad");
        for text in [
            "not json",
            r#"{"origins":"wss://x"}"#,
            r#"{"origins":["https://x"]}"#,
            r#"{"origins":["wss://x; img-src *"]}"#,
            r#"{"origins":[1]}"#,
            "{}",
        ] {
            std::fs::write(d.join(CONNECT_ORIGINS_FILE), text).unwrap();
            assert!(load_connect_origins(&d).is_err(), "{text}");
        }
        std::fs::remove_dir_all(d).unwrap();
    }
}
