//! Generic URL preset loading and matching.
//!
//! Presets are caller-supplied runtime hints. This module only parses their
//! configuration, matches URL globs, and formats suggested CLI commands.

use serde::Deserialize;
use std::path::Path;

/// Versioned collection of URL presets, evaluated from top to bottom.
#[derive(Debug, Clone, Deserialize)]
pub struct PresetFile {
    /// Preset file format version.
    pub version: u32,
    /// URL presets in first-match-wins order.
    #[serde(default)]
    pub presets: Vec<UrlPreset>,
}

/// Runtime hints associated with a URL glob.
#[derive(Debug, Clone, Deserialize)]
pub struct UrlPreset {
    /// URL glob, where `*` is the only wildcard.
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
    /// Preferred impersonation target for engines that support this hint.
    #[serde(default)]
    pub impersonate_first: Option<String>,
    /// Preferred referer strategy for engines that support this hint.
    #[serde(default)]
    pub referer_strategy: Option<String>,
}

/// Load and parse a preset file, returning errors as fail-soft messages.
pub fn load(path: &Path) -> Result<PresetFile, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    toml::from_str(&contents)
        .map_err(|error| format!("failed to parse {}: {error}", path.display()))
}

impl PresetFile {
    /// Return the first preset whose glob matches `url`.
    pub fn match_url(&self, url: &str) -> Option<&UrlPreset> {
        self.presets
            .iter()
            .find(|preset| glob_match(&preset.r#match, url))
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

fn shell_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}

fn glob_match(pattern: &str, text: &str) -> bool {
    // URLs are effectively case-insensitive for preset routing, so normalize both
    // sides before matching. Character vectors keep non-ASCII literals intact.
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
        assert!(glob_match("SCHEME://HOST/*", "scheme://host/Path"));
    }

    #[test]
    fn match_url_returns_the_first_matching_preset() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("*target*", "first"), preset("*", "second")],
        };

        assert_eq!(
            file.match_url("scheme://target/path")
                .and_then(|matched| matched.reason.as_deref()),
            Some("first")
        );
    }

    #[test]
    fn match_url_returns_none_without_a_match() {
        let file = PresetFile {
            version: 1,
            presets: vec![preset("prefix-*", "only")],
        };

        assert!(file.match_url("other-value").is_none());
    }

    #[test]
    fn suggested_command_omits_auto_device_and_quotes_url() {
        let mut value = preset("*", "reason");
        value.device = Some("auto".to_string());

        assert_eq!(
            value.suggested_command("scheme://host/path"),
            "adaptive-fetch \"scheme://host/path\""
        );
    }

    #[test]
    fn suggested_command_includes_device_and_repeated_selectors() {
        let mut value = preset("*", "reason");
        value.device = Some("desktop".to_string());
        value.selectors = vec!["main".to_string(), "article.body".to_string()];

        assert_eq!(
            value.suggested_command("scheme://host/path"),
            "adaptive-fetch \"scheme://host/path\" --device desktop --selector \"main\" --selector \"article.body\""
        );
    }

    #[test]
    fn suggested_command_escapes_quotes_and_backslashes() {
        let mut value = preset("*", "reason");
        value.selectors = vec![r#"[data-label="quoted"]\path"#.to_string()];

        assert_eq!(
            value.suggested_command(r#"scheme://host/"quoted"\path"#),
            r#"adaptive-fetch "scheme://host/\"quoted\"\\path" --selector "[data-label=\"quoted\"]\\path""#
        );
    }

    #[test]
    fn parses_inline_document_and_loads_it_from_disk() {
        let document = r#"
version = 1

[[presets]]
match = "*://*/*"
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
