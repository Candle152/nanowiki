//! Configuration module — multi-provider profiles + interactive setup

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;


#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub default_provider: Option<String>,
    /// currently active model (must exist in default_provider models list)
    #[serde(default)]
    pub current_model: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub api_key: String,
    pub models: Vec<String>,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    OpenAI,
    Anthropic,
}

impl Config {
    /// resolve the current provider config
    pub fn resolve(&self) -> anyhow::Result<(&str, &ProviderConfig)> {
        let name = self
            .default_provider
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("no default provider configured"))?;
        let pc = self
            .providers
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("provider '{}' not found", name))?;
        Ok((name, pc))
    }

    /// resolve current model: current_model > provider models[0]
    pub fn resolve_model(&self, pc: &ProviderConfig) -> String {
        self.current_model
            .as_deref()
            .filter(|m| pc.models.iter().any(|x| x == m))
            .or_else(|| pc.models.first().map(|s| s.as_str()))
            .unwrap_or("?")
            .to_string()
    }

    pub fn provider_type_label(&self, name: &str) -> &str {
        match self.providers.get(name).map(|p| p.provider_type) {
            Some(ProviderType::OpenAI) => "OpenAI",
            Some(ProviderType::Anthropic) => "Anthropic",
            None => "Unknown",
        }
    }

    pub fn effective_base_url(pc: &ProviderConfig) -> &str {
        pc.base_url.as_deref().unwrap_or(match pc.provider_type {
            ProviderType::OpenAI => "https://api.openai.com/v1",
            ProviderType::Anthropic => "https://api.anthropic.com",
        })
    }
}


fn config_dir() -> PathBuf {
    dirs_fallback::home_dir().join(".nanowiki")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}


pub fn load_or_create() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return interactive_setup();
    }
    let content = std::fs::read_to_string(&path)?;
    serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("config parse error {}: {}", path.display(), e))
}

fn interactive_setup() -> anyhow::Result<Config> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)?;

    println!();
    println!("============================================");
    println!("        NanoWiki - First-time Setup");
    println!("============================================");
    println!();
    println!("We will configure an LLM provider. Press Ctrl+C to exit.");
    println!();

    let mut config = Config {
        default_provider: None,
        current_model: None,
        providers: HashMap::new(),
    };

    loop {
        let name = prompt("Provider name (e.g. openai / deepseek / claude)")?;

        println!("  [1] OpenAI-compatible  [2] Anthropic native");
        let pt = match prompt_default("  Choice", "1")?.trim() {
            "2" => ProviderType::Anthropic,
            _ => ProviderType::OpenAI,
        };

        let default_models = match pt {
            ProviderType::OpenAI => "gpt-4o, gpt-4o-mini",
            ProviderType::Anthropic => "claude-sonnet-4-20250514",
        };
        let models: Vec<String> = prompt_default(
            &format!("  Models (comma-separated)[{}]", default_models),
            default_models,
        )?
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

        let default_url = match pt {
            ProviderType::OpenAI => "https://api.openai.com/v1",
            ProviderType::Anthropic => "https://api.anthropic.com",
        };
        let url = prompt_default(&format!("  Base URL [{}]", default_url), default_url)?;
        let base_url = if url == default_url { None } else { Some(url) };

        let api_key = prompt("  API Key")?;

        config.providers.insert(
            name.clone(),
            ProviderConfig { provider_type: pt, api_key, models, base_url },
        );

        if config.default_provider.is_none()
            && prompt_default("  Set as default? [Y/n]", "Y")?.to_lowercase().starts_with('y') {
                config.default_provider = Some(name.clone());
                config.current_model = config.providers[&name].models.first().cloned();
            }

        println!();
        if !prompt_default("  Add another provider? [y/N]", "N")?.to_lowercase().starts_with('y') {
            break;
        }
        println!();
    }

    if config.default_provider.is_none()
        && let Some(name) = config.providers.keys().next() {
            config.default_provider = Some(name.clone());
            config.current_model = config.providers[name].models.first().cloned();
        }

    validate(&config)?;
    save(&config)?;
    println!();
    println!("✅ Config saved to {}", config_path().display());
    println!();

    Ok(config)
}


/// find the provider for a model name and switch to it
pub fn switch_by_model(config: &mut Config, model: &str) -> anyhow::Result<()> {
    for (name, pc) in &config.providers {
        if pc.models.iter().any(|m| m == model) {
            config.default_provider = Some(name.clone());
            config.current_model = Some(model.to_string());
            return Ok(());
        }
    }
    anyhow::bail!("model '{}' not found in any provider", model)
}

/// switch provider (resets model to provider models[0])
pub fn switch_by_provider(config: &mut Config, name: &str) -> anyhow::Result<()> {
    let pc = config
        .providers
        .get(name)
        .ok_or_else(|| anyhow::anyhow!("provider '{}' not found", name))?;
    config.default_provider = Some(name.to_string());
    config.current_model = pc.models.first().cloned();
    Ok(())
}


pub fn validate(config: &Config) -> anyhow::Result<()> {
    if config.providers.is_empty() {
        anyhow::bail!("at least one provider is required");
    }
    for (name, p) in &config.providers {
        if p.api_key.trim().is_empty() || p.api_key.starts_with("sk-your-") {
            anyhow::bail!("provider '{}' api_key is not set or is a placeholder", name);
        }
        if p.models.is_empty() {
            anyhow::bail!("provider '{}' models list cannot be empty", name);
        }
    }
    Ok(())
}

pub fn save(config: &Config) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    std::fs::write(config_path(), json)?;
    Ok(())
}


fn prompt(label: &str) -> anyhow::Result<String> {
    use std::io::{stdin, stdout, Write};
    let mut input = String::new();
    print!("{}: ", label);
    stdout().flush()?;
    stdin().read_line(&mut input)?;
    let t = input.trim().to_string();
    if t.is_empty() { anyhow::bail!("input cannot be empty"); }
    Ok(t)
}

fn prompt_default(label: &str, default: &str) -> anyhow::Result<String> {
    use std::io::{stdin, stdout, Write};
    let mut input = String::new();
    print!("{} ", label);
    stdout().flush()?;
    stdin().read_line(&mut input)?;
    let t = input.trim().to_string();
    Ok(if t.is_empty() { default.to_string() } else { t })
}


mod dirs_fallback {
    use std::path::PathBuf;
    pub fn home_dir() -> PathBuf {
        std::env::var("NANOWIKI_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("USERPROFILE")
                    .or_else(|_| std::env::var("HOME"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn mk_config(providers: Vec<(&str, Vec<&str>)>, default: &str, model: &str) -> Config {
        Config {
            default_provider: Some(default.to_string()),
            current_model: Some(model.to_string()),
            providers: providers
                .into_iter()
                .map(|(k, v)| {
                    (k.to_string(), ProviderConfig {
                        provider_type: ProviderType::OpenAI,
                        api_key: "k".into(),
                        models: v.into_iter().map(|s| s.to_string()).collect(),
                        base_url: None,
                    })
                })
                .collect(),
        }
    }

    #[test]
    fn resolve_basic() {
        let cfg = mk_config(vec![("o", vec!["m1", "m2"])], "o", "m1");
        let (name, _) = cfg.resolve().unwrap();
        assert_eq!(name, "o");
    }

    #[test]
    fn resolve_model_uses_current() {
        let cfg = mk_config(vec![("o", vec!["m1", "m2"])], "o", "m2");
        let (_, pc) = cfg.resolve().unwrap();
        assert_eq!(cfg.resolve_model(pc), "m2".to_string());
    }

    #[test]
    fn resolve_model_falls_back_to_first() {
        let cfg = mk_config(vec![("o", vec!["m1", "m2"])], "o", "x");
        let (_, pc) = cfg.resolve().unwrap();
        assert_eq!(cfg.resolve_model(pc), "m1".to_string()); // current not in list, fallback
    }

    #[test]
    fn switch_by_model_finds_provider() {
        let mut cfg = mk_config(vec![("a", vec!["m1"]), ("b", vec!["m2"])], "a", "m1");
        switch_by_model(&mut cfg, "m2").unwrap();
        assert_eq!(cfg.default_provider.unwrap(), "b");
        assert_eq!(cfg.current_model.unwrap(), "m2");
    }

    #[test]
    fn switch_by_model_unknown() {
        let mut cfg = mk_config(vec![("a", vec!["m1"])], "a", "m1");
        assert!(switch_by_model(&mut cfg, "m2").is_err());
    }

    #[test]
    fn switch_by_provider_resets_model() {
        let mut cfg = mk_config(vec![("a", vec!["m1", "m2"])], "a", "m2");
        switch_by_provider(&mut cfg, "a").unwrap();
        assert_eq!(cfg.current_model.unwrap(), "m1"); // reset to first
    }

    #[test]
    fn validate_ok() {
        let cfg = mk_config(vec![("a", vec!["m"])], "a", "m");
        assert!(validate(&cfg).is_ok());
    }
}
