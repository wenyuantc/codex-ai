use serde::{Deserialize, Serialize};

use crate::db::models::ChannelModelConfig;

const CATALOG_JSON: &str = include_str!("model_catalog.json");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub vendor: String,
    pub label: String,
    pub context_tokens: u32,
    pub max_output_tokens: u32,
    pub thinking: bool,
    #[serde(default)]
    pub thinking_levels: Vec<String>,
}

fn catalog_entries() -> &'static [ModelCatalogEntry] {
    use std::sync::OnceLock;
    static ENTRIES: OnceLock<Vec<ModelCatalogEntry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        serde_json::from_str(CATALOG_JSON).expect("bundled model catalog must parse")
    })
}

pub fn list_catalog() -> Vec<ModelCatalogEntry> {
    catalog_entries().to_vec()
}

fn normalize_model_key(value: &str) -> String {
    value
        .trim()
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(value)
        .trim()
        .replace('_', "-")
        .to_ascii_lowercase()
}

pub fn lookup_catalog(model_id: &str) -> Option<&'static ModelCatalogEntry> {
    let raw = model_id.trim();
    if raw.is_empty() {
        return None;
    }
    let key = normalize_model_key(raw);
    let entries = catalog_entries();
    if let Some(exact) = entries.iter().find(|entry| {
        entry.id.eq_ignore_ascii_case(raw)
            || entry
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(raw))
    }) {
        return Some(exact);
    }
    if let Some(exact_key) = entries.iter().find(|entry| {
        normalize_model_key(&entry.id) == key
            || entry
                .aliases
                .iter()
                .any(|alias| normalize_model_key(alias) == key)
    }) {
        return Some(exact_key);
    }
    entries
        .iter()
        .filter(|entry| {
            let catalog_key = normalize_model_key(&entry.id);
            catalog_key.len() >= 6
                && (key.starts_with(&catalog_key) || catalog_key.starts_with(&key))
        })
        .max_by_key(|entry| normalize_model_key(&entry.id).len())
}

pub fn apply_catalog_defaults(model_id: &str) -> ChannelModelConfig {
    let mut config = ChannelModelConfig {
        id: model_id.trim().to_string(),
        context_tokens: None,
        max_output_tokens: None,
        thinking_enabled: None,
        thinking_level: None,
        thinking_levels: None,
    };
    fill_from_catalog(&mut config);
    config
}

pub fn fill_from_catalog(config: &mut ChannelModelConfig) {
    let Some(entry) = lookup_catalog(&config.id) else {
        return;
    };
    if config.context_tokens.is_none() {
        config.context_tokens = Some(entry.context_tokens);
    }
    if config.max_output_tokens.is_none() {
        config.max_output_tokens = Some(entry.max_output_tokens);
    }
    if config.thinking_enabled.is_none() {
        config.thinking_enabled = Some(entry.thinking);
    }
    if config.thinking_level.is_none() {
        config.thinking_level = if entry.thinking {
            entry
                .thinking_levels
                .iter()
                .find(|level| *level == "medium")
                .cloned()
                .or_else(|| entry.thinking_levels.first().cloned())
        } else {
            None
        };
    }
    let stored_levels = config.thinking_levels.as_deref().unwrap_or(&[]);
    let catalog_has_new_level = entry
        .thinking_levels
        .iter()
        .any(|level| !stored_levels.iter().any(|item| item == level));
    if stored_levels.is_empty() || catalog_has_new_level {
        config.thinking_levels = Some(entry.thinking_levels.clone());
    }
}

#[tauri::command]
pub fn list_model_catalog() -> Vec<ModelCatalogEntry> {
    list_catalog()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_requested_vendors() {
        let catalog = list_catalog();
        let vendors: Vec<&str> = catalog.iter().map(|item| item.vendor.as_str()).collect();
        for vendor in [
            "openai",
            "anthropic",
            "deepseek",
            "minimax",
            "glm",
            "kimi",
            "doubao",
            "hunyuan",
            "gemini",
            "mimo",
        ] {
            assert!(vendors.contains(&vendor), "missing vendor {vendor}");
        }
    }

    #[test]
    fn lookup_matches_aliases_and_prefixed_ids() {
        assert_eq!(lookup_catalog("gpt-4o").unwrap().context_tokens, 128000);
        assert_eq!(
            lookup_catalog("deepseek-ai/DeepSeek-V3").unwrap().id,
            "deepseek-chat"
        );
        assert_eq!(
            lookup_catalog("claude-sonnet-4-6-20260217").unwrap().id,
            "claude-sonnet-4-6"
        );
        assert!(lookup_catalog("totally-unknown-model-xyz").is_none());
    }

    #[test]
    fn gpt_5_6_luna_includes_xhigh_and_max() {
        let entry = lookup_catalog("gpt-5.6-luna").expect("luna");
        assert!(entry.thinking_levels.contains(&"xhigh".to_string()));
        assert!(entry.thinking_levels.contains(&"max".to_string()));
        let mut config = apply_catalog_defaults("gpt-5.6-luna");
        config.thinking_levels = Some(vec![
            "minimal".to_string(),
            "low".to_string(),
            "medium".to_string(),
            "high".to_string(),
        ]);
        fill_from_catalog(&mut config);
        assert_eq!(
            config.thinking_levels.as_deref(),
            Some(
                [
                    "low".to_string(),
                    "medium".to_string(),
                    "high".to_string(),
                    "xhigh".to_string(),
                    "max".to_string()
                ]
                .as_slice()
            )
        );
    }
}
