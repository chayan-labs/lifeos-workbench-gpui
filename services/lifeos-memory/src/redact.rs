//! The no-secret-in-memory guard (docs/AI-MEMORY.md §9, SECURITY.md).
//!
//! Applied at PROJECTION time - the only door through which event payloads
//! become memory rows - so a secret that slips into an event's attrs can
//! still never surface via recall. Two layers:
//! 1. whole event types that never become memories (credential lifecycles),
//! 2. key-name redaction inside any attrs object that does project.

/// Event-type prefixes that never project into memory at all.
const SKIP_TYPE_PREFIXES: &[&str] = &[
    "connection.", // OAuth/credential lifecycle
    "auth.",
    "session.",
    "login.",
    "secret.",
];

/// Attr keys whose values are dropped wherever they appear (case-insensitive
/// substring match on the key name).
const SECRET_KEY_MARKERS: &[&str] = &[
    "token", "secret", "password", "passwd", "api_key", "apikey", "credential",
    "authorization", "private_key", "access_key", "cookie",
];

/// Should this event type be excluded from memory entirely?
pub fn is_secret_event_type(event_type: &str) -> bool {
    SKIP_TYPE_PREFIXES.iter().any(|p| event_type.starts_with(p))
}

fn is_secret_key(key: &str) -> bool {
    let lower = key.to_lowercase();
    SECRET_KEY_MARKERS.iter().any(|m| lower.contains(m))
}

/// Flatten a JSON attrs value into searchable text, dropping secret-named
/// keys (and everything under them) recursively. Deterministic: object keys
/// are visited in sorted order so rebuilds reproduce identical content.
pub fn flatten_redacted(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for key in keys {
                if is_secret_key(key) {
                    continue;
                }
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(key);
                out.push_str(": ");
                flatten_redacted(&map[key], out);
            }
        }
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                flatten_redacted(item, out);
            }
        }
        serde_json::Value::String(s) => out.push_str(s),
        other => out.push_str(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn secret_keys_are_dropped_recursively() {
        let attrs = json!({
            "note": "call the broker",
            "api_key": "sk-SUPER-SECRET",
            "nested": { "refresh_token": "rt-SECRET", "detail": "visible" },
            "Authorization": "Bearer xyz"
        });
        let mut out = String::new();
        flatten_redacted(&attrs, &mut out);
        assert!(out.contains("call the broker"));
        assert!(out.contains("visible"));
        assert!(!out.contains("SECRET"));
        assert!(!out.contains("Bearer"));
    }

    #[test]
    fn credential_event_types_are_skipped_entirely() {
        assert!(is_secret_event_type("connection.created"));
        assert!(is_secret_event_type("auth.password.changed"));
        assert!(!is_secret_event_type("task.completed"));
    }

    #[test]
    fn flatten_is_deterministic_across_key_order() {
        let a = json!({"b": "two", "a": "one"});
        let b = json!({"a": "one", "b": "two"});
        let (mut oa, mut ob) = (String::new(), String::new());
        flatten_redacted(&a, &mut oa);
        flatten_redacted(&b, &mut ob);
        assert_eq!(oa, ob);
    }
}
