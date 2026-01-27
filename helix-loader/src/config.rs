use std::str::from_utf8;

use crate::workspace_trust::{TrustQuery, WorkspaceTrust};

/// Default built-in languages.toml.
pub fn default_lang_config() -> toml::Value {
    let default_config = include_bytes!("../../languages.toml");
    toml::from_str(from_utf8(default_config).unwrap())
        .expect("Could not parse built-in languages.toml to valid toml")
}

fn merge_lang_config_values(left: toml::Value, mut right: toml::Value) -> toml::Value {
    if let (Some(left_servers), Some(right_servers)) = (
        left.get("language-server").and_then(toml::Value::as_table),
        right
            .get_mut("language-server")
            .and_then(toml::Value::as_table_mut),
    ) {
        for (name, right_server) in right_servers {
            let Some(left_config) = left_servers
                .get(name)
                .and_then(|server| server.get("config"))
            else {
                continue;
            };
            let Some(right_config) = right_server.get_mut("config") else {
                continue;
            };
            *right_config = crate::merge_toml_values(left_config.clone(), right_config.clone(), 10);
        }
    }

    crate::merge_toml_values(left, right, 3)
}

/// User configured languages.toml file, merged with the default config.
///
/// Workspace-local `.helix/languages.toml` is merged in only when the current
/// workspace is trusted for [`TrustQuery::LocalConfig`].
pub fn user_lang_config(trust: &WorkspaceTrust) -> Result<toml::Value, toml::de::Error> {
    let global_config = crate::lang_config_file();
    let workspace_config = crate::workspace_lang_config_file();

    let files = if trust.query_current(TrustQuery::LocalConfig).is_trusted() {
        vec![global_config, workspace_config]
    } else {
        vec![global_config]
    };

    let config = files
        .iter()
        .filter_map(|file| {
            std::fs::read_to_string(file)
                .map(|config| toml::from_str(&config))
                .ok()
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .fold(default_lang_config(), merge_lang_config_values);

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::merge_lang_config_values;

    #[test]
    fn language_server_config_merges_deeply_without_merging_language_server_lists() {
        let default = toml::from_str(
            r#"
            [language-server.example]
            command = "example"
            args = ["default"]

            [language-server.example.config]
            inherited = true

            [language-server.example.config.nested]
            inherited = true
            overridden = "default"

            [[language]]
            name = "json"
            scope = "source.json"
            language-servers = ["default"]
            "#,
        )
        .unwrap();
        let user = toml::from_str(
            r#"
            [language-server.example]
            args = ["user"]

            [language-server.example.config]
            added = true

            [language-server.example.config.nested]
            overridden = "user"

            [[language]]
            name = "json"
            language-servers = ["user"]
            "#,
        )
        .unwrap();
        let expected = toml::from_str(
            r#"
            [language-server.example]
            command = "example"
            args = ["user"]

            [language-server.example.config]
            inherited = true
            added = true

            [language-server.example.config.nested]
            inherited = true
            overridden = "user"

            [[language]]
            name = "json"
            scope = "source.json"
            language-servers = ["user"]
            "#,
        )
        .unwrap();

        assert_eq!(merge_lang_config_values(default, user), expected);
    }
}

/// Default built-in auto-pairs.toml.
pub fn default_auto_pairs_config() -> toml::Value {
    let default_config = include_bytes!("../../auto-pairs.toml");
    toml::from_str(from_utf8(default_config).unwrap())
        .expect("Could not parse built-in auto-pairs.toml to valid toml")
}

/// Error type for auto-pairs config loading.
#[derive(Debug)]
pub struct AutoPairsConfigError {
    pub path: std::path::PathBuf,
    pub error: toml::de::Error,
}

impl std::fmt::Display for AutoPairsConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.error)
    }
}

impl std::error::Error for AutoPairsConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// Load auto-pairs config, merged with user overrides.
///
/// Priority (lowest to highest):
/// 1. Built-in auto-pairs.toml (embedded)
/// 2. User ~/.config/helix/auto-pairs.toml
/// 3. Workspace .helix/auto-pairs.toml
///
/// Note: Explicit `auto-pairs` in languages.toml takes precedence over all of these.
pub fn auto_pairs_config() -> Result<toml::Value, AutoPairsConfigError> {
    let mut configs = Vec::new();

    for path in [
        crate::config_dir(),
        crate::find_workspace().0.join(".helix"),
    ] {
        let file = path.join("auto-pairs.toml");
        if let Ok(content) = std::fs::read_to_string(&file) {
            let parsed: toml::Value =
                toml::from_str(&content).map_err(|error| AutoPairsConfigError {
                    path: file.clone(),
                    error,
                })?;
            configs.push(parsed);
        }
    }

    let config = configs
        .into_iter()
        .fold(default_auto_pairs_config(), |a, b| {
            crate::merge_toml_values(a, b, 1)
        });

    Ok(config)
}
