//! Generic URL preset loading and matching.
//!
//! Presets are caller-supplied runtime hints. This module only parses their
//! configuration, matches a URL's **hostname** against glob patterns, and formats
//! suggested CLI commands. Matching is host-scoped (not full-URL) so a pattern
//! cannot cross path boundaries and bare origins still match.

use serde::Deserialize;
use std::path::Path;

/// Device values the `adaptive-fetch` CLI accepts. A preset outside this set would
/// produce a suggested command that fails clap validation, so it is rejected at
/// load time instead of creating a dead end (deny + unrunnable command).
const VALID_DEVICES: [&str; 3] = ["auto", "desktop", "mobile"];

/// Versioned collection of URL presets, evaluated from top to bottom.
#[derive(Debug, Clone, Deserialize)]
pub struct PresetFile {
    /// Preset file format version.
    pub version: u32,
    /// URL presets in first-match-wins order.
    #[serde(default)]
    pub presets: Vec<UrlPreset>,
}

/// Runtime hints associated with a host glob.
#[derive(Debug, Clone, Deserialize)]
pub struct UrlPreset {
    /// Host glob, where `*` is the only wildcard, matched against the URL hostname.
    pub r#match: String,
    /// Explanation shown when the preset matches.
    #[serde(default)]
    pub reason: Option<String>,
    /// Device shaping hint (`auto`, `desktop`, or `mobile`).
    #[serde(default)]
    pub device: Option<String>,
    /// Success selectors forwarded to the CLI.
    #[serde(default)]
    pub selectors: Vec<String>,
    /// Preferred impersonation target — reserved for a later milestone (stored, not
    /// yet emitted as a CLI flag).
    #[serde(default)]
    pub impersonate_first: Option<String>,
    /// Preferred referer strategy — reserved for a later milestone (stored, not yet
    /// emitted as a CLI flag).
    #[serde(default)]
    pub referer_strategy: Option<String>,
}

/// Load, parse, and validate a preset file, returning errors as fail-soft messages.
pub fn load(path: &Path) -> Result<PresetFile, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let file: PresetFile = toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))?;
    file.validate()
        .map_err(|error| format!("invalid preset in {}: {error}", path.display()))?;
    Ok(file)
}

impl PresetFile {
    /// Return the first preset whose glob matches the hostname of `url`.
    pub fn match_url(&self, url: &str) -> Option<&UrlPreset> {
        let host = extract_host(url)?;
        self.presets
            .iter()
            .find(|preset| glob_match(&preset.r#match, host))
    }

    /// Reject presets whose `device` the CLI would not accept.
    pub fn validate(&self) -> Result<(), String> {
        for preset in &self.presets {
            if let Some(device) = &preset.device {
                if !VALID_DEVICES.contains(&device.as_str()) {
                    return Err(format!(
                        "device \"{device}\" for match \"{}\" is not one of {}",
                        preset.r#match,
                        VALID_DEVICES.join(", ")
                    ));
                }
            }
        }
        Ok(())
    }
}

impl UrlPreset {
    /// Build a runnable `adaptive-fetch` command for `url`.
    pub fn suggested_command(&self, url: &str) -> String {
        let mut command = format!("adaptive-fetch {}", shell_quote(url));

        if let Some(device) = &self.device {
            if device != "auto" {
                command.push_str(&format!(" --device {device}"));
            }
        }

        for selector in &self.selectors {
            command.push_str(&format!(" --selector {}", shell_quote(selector)));
        }

        command
    }
}

/// Extract the lowercase-comparable hostname from `url` (scheme, userinfo, port,
/// and path stripped). Returns `None` when no host is present.
fn extract_host(url: &str) -> Option<&str> {
    let after_scheme = match url.find("://") {
        Some(index) => &url[index + 3..],
        None => url,
    };
    let authority_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    // Drop any userinfo (`user:pass@`).
    let host_port = match authority.rfind('@') {
        Some(index) => &authority[index + 1..],
        None => authority,
    };
    // Drop the port. Bracketed IPv6 literals keep their inner colons.
    let host = if let Some(inner) = host_port.strip_prefix('[') {
        inner.split(']').next().unwrap_or(host_port)
    } else {
        match host_port.rfind(':') {
            Some(index) => &host_port[..index],
            None => host_port,
        }
    };
    // A trailing DNS root dot (`reddit.com.`) resolves to the same host, so strip it
    // before matching.
    let host = host.strip_suffix('.').unwrap_or(host);

    (!host.is_empty()).then_some(host)
}

fn shell_quote(value: &str) -> String {
    // POSIX single-quote escaping. Single quotes suppress *all* shell evaluation —
    // `$(...)`, backticks, `$VAR` — so a hostile URL or selector cannot inject a
    // command into the suggested string. A literal single quote closes the quote,
    // emits an escaped `'`, and reopens: `'\''`.
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // Hostnames are case-insensitive, so normalize both sides before matching.
    // Character vectors keep non-ASCII literals intact.
    let pattern: Vec<char> = pattern.to_lowercase().chars().collect();
    let text: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pattern_index, mut text_index) = (0, 0);
    let mut star_index = None;
    let mut star_text_index = 0;

    while text_index < text.len() {
        if pattern_index < pattern.len() && pattern[pattern_index] == text[text_index] {
            pattern_index += 1;
            text_index += 1;
        } else if pattern_index < pattern.len() && pattern[pattern_index] == '*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            star_text_index = text_index;
        } else if let Some(star) = star_index {
            pattern_index = star + 1;
            star_text_index += 1;
            text_index = star_text_index;
        } else {
            return false;
        }
    }

    while pattern_index < pattern.len() && pattern[pattern_index] == '*' {
        pattern_index += 1;
    }

    pattern_index == pattern.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(pattern: &str, reason: &str) -> UrlPreset {
        UrlPreset {
            r#match: pattern.to_string(),
            reason: Some(reason.to_string()),
            device: None,
            selectors: Vec::new(),
            impersonate_first: None,
            referer_strategy: None,
        }
    }

    #[test]
    fn glob_matching_supports_only_star_wildcards() {
        assert!(glob_match("*suffix", "prefix-suffix"));
        assert!(glob_match("prefix*", "prefix-suffix"));
        assert!(glob_match("prefix*suffix", "prefix-middle-suffix"));
        assert!(glob_match("*one*two*", "zero-one-middle-two-end"));
        assert!(glob_match("literal", "literal"));
        assert!(!glob_match("prefix*suffix", "prefix-middle"));
    }

    #[test]
    fn glob_matching_is_case_insensitive() {
        assert!(glob_match("HOST.EXAMPLE", "host.example"));
    }

    #[test]
    fn extract_host_strips_scheme_userinfo_port_and_path() {
        assert_eq!(
            extract_host("https://www.example.com/path?q=1"),
            Some("www.example.com")
        );
        assert_eq!(
            extract_host("https://user:pass@example.com:8443/x"),
            Some("example.com")
        );
        assert_eq!(
            extract_host("https://[2001:db8::1]:443/x"),
            Some("2001:db8::1")
        );
        assert_eq!(extract_host("example.com"), Some("example.com"));
        assert_eq!(extract_host("https:///path"), None);
    }

    #[test]
    fn extract_host_drops_trailing_dns_root_dot() {
        assert_eq!(
            extract_host("https://reddit.com./r/rust"),
            Some("reddit.com")
        );
        assert_eq!(
            extract_host("https://www.reddit.com."),
            Some("www.reddit.com")
        );
    }

    #[test]
    fn match_url_matches_host_not_path() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("*.example.com", "sub")],
        };

        // A real sub-domain matches.
        assert!(file.match_url("https://www.example.com/path").is_some());
        // The pattern embedded in an unrelated host's PATH must not match — host
        // matching stops the glob from crossing the `/` boundary.
        assert!(file
            .match_url("https://evil.com/path/.example.com/x")
            .is_none());
        // The apex is deliberately not covered by a sub-domain pattern.
        assert!(file.match_url("https://example.com/").is_none());
    }

    #[test]
    fn match_url_matches_bare_origin_without_trailing_slash() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("example.com", "apex")],
        };

        assert!(file.match_url("https://example.com").is_some());
        assert!(file.match_url("https://example.com/some/page").is_some());
    }

    #[test]
    fn match_url_returns_the_first_matching_preset() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("target.example", "first"), preset("*", "second")],
        };

        assert_eq!(
            file.match_url("scheme://target.example/path")
                .and_then(|matched| matched.reason.as_deref()),
            Some("first")
        );
    }

    #[test]
    fn match_url_returns_none_without_a_match() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("prefix.example", "only")],
        };

        assert!(file.match_url("https://other.example/").is_none());
    }

    #[test]
    fn validate_rejects_unknown_device() {
        let mut bad = preset("x.example", "reason");
        bad.device = Some("tablet".to_string());
        let file = PresetFile {
            version: 1,
            presets: vec![bad],
        };

        let error = file
            .validate()
            .expect_err("unknown device should be rejected");
        assert!(
            error.contains("tablet"),
            "error should name the bad value: {error}"
        );
    }

    #[test]
    fn validate_accepts_known_devices() {
        for device in ["auto", "desktop", "mobile"] {
            let mut ok = preset("x.example", "reason");
            ok.device = Some(device.to_string());
            let file = PresetFile {
                version: 1,
                presets: vec![ok],
            };
            assert!(file.validate().is_ok(), "{device} should be accepted");
        }
    }

    #[test]
    fn suggested_command_omits_auto_device_and_quotes_url() {
        let mut value = preset("*", "reason");
        value.device = Some("auto".to_string());

        assert_eq!(
            value.suggested_command("scheme://host/path"),
            "adaptive-fetch 'scheme://host/path'"
        );
    }

    #[test]
    fn suggested_command_includes_device_and_repeated_selectors() {
        let mut value = preset("*", "reason");
        value.device = Some("desktop".to_string());
        value.selectors = vec!["main".to_string(), "article.body".to_string()];

        assert_eq!(
            value.suggested_command("scheme://host/path"),
            "adaptive-fetch 'scheme://host/path' --device desktop --selector 'main' --selector 'article.body'"
        );
    }

    #[test]
    fn suggested_command_neutralizes_shell_metacharacters() {
        let mut value = preset("*", "reason");
        value.selectors = vec!["a'b".to_string()];

        // The `$(...)` in the URL and the single quote in the selector are both
        // wrapped in single quotes, so nothing is evaluated when the command runs.
        assert_eq!(
            value.suggested_command("https://host/$(whoami)`id`"),
            r#"adaptive-fetch 'https://host/$(whoami)`id`' --selector 'a'\''b'"#
        );
    }

    #[test]
    fn parses_inline_document_and_loads_it_from_disk() {
        let document = r#"
version = 1

[[presets]]
match = "*.example.com"
reason = "Runtime hint"
device = "mobile"
selectors = ["main", "article"]
impersonate_first = "profile"
referer_strategy = "self_root"
"#;
        let parsed: PresetFile = toml::from_str(document).expect("inline preset should parse");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.presets.len(), 1);
        assert_eq!(parsed.presets[0].selectors, ["main", "article"]);

        let path = std::env::temp_dir().join(format!(
            "adaptive-fetch-presets-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should follow the Unix epoch")
                .as_nanos()
        ));
        std::fs::write(&path, document).expect("temporary preset should be writable");
        let loaded = load(&path).expect("temporary preset should load");
        std::fs::remove_file(path).expect("temporary preset should be removable");

        assert_eq!(loaded.presets[0].device.as_deref(), Some("mobile"));
        assert_eq!(
            loaded.presets[0].impersonate_first.as_deref(),
            Some("profile")
        );
        assert_eq!(
            loaded.presets[0].referer_strategy.as_deref(),
            Some("self_root")
        );
    }
}
