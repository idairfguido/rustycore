//! WoW server `.conf` file parser.
//!
//! Parses configuration files that use the `Key = Value` format found in
//! TrinityCore/RustyCore `.conf.dist` files. Provides a global singleton
//! [`ConfigMgr`] for application-wide configuration access.
//!
//! # Format
//!
//! ```text
//! # This is a comment
//! DataDir = "/home/server/data"
//! WorldServerPort = 8085
//! Rate.XP.Kill = 1.5
//! ```

use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur while loading or parsing a configuration file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The configuration file could not be found or read.
    #[error("config file not found: {0}")]
    FileNotFound(String),

    /// None of the requested configuration files could be loaded.
    #[error("no config file found; tried: {0}")]
    NoConfigFile(String),

    /// A line in the configuration file could not be parsed.
    #[error("parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    /// A database connection string could not be parsed.
    #[error("invalid {key}: {message}")]
    InvalidDatabaseInfo { key: String, message: String },
}

// ---------------------------------------------------------------------------
// Internal config store
// ---------------------------------------------------------------------------

/// Internal configuration store.
///
/// Keys are stored in **lowercase** so that lookups are case-insensitive.
#[derive(Debug, Default)]
struct ConfigStore {
    values: HashMap<String, ConfigEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigEntry {
    original_key: String,
    value: String,
}

impl ConfigStore {
    /// Parse the full text content of a `.conf` file into the store.
    fn parse(&mut self, content: &str) -> Result<(), ConfigError> {
        self.values.clear();
        self.merge(content)
    }

    /// Merge the full text content of a `.conf` file into the store.
    fn merge(&mut self, content: &str) -> Result<(), ConfigError> {
        for (idx, raw_line) in content.lines().enumerate() {
            let line_number = idx + 1;
            let line = raw_line.trim();

            // Skip empty lines and full-line comments.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Find the first '=' to split key and value.
            let Some(eq_pos) = line.find('=') else {
                return Err(ConfigError::ParseError {
                    line: line_number,
                    message: format!("expected '=' in: {line}"),
                });
            };

            let key = line[..eq_pos].trim();
            if key.is_empty() {
                return Err(ConfigError::ParseError {
                    line: line_number,
                    message: "empty key".to_string(),
                });
            }

            let raw_value = line[eq_pos + 1..].trim();
            let value = parse_value(raw_value);

            self.values.insert(
                key.to_ascii_lowercase(),
                ConfigEntry {
                    original_key: key.to_string(),
                    value,
                },
            );
        }

        Ok(())
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.values
            .get(&key.to_ascii_lowercase())
            .map(|entry| entry.value.as_str())
    }

    fn override_with_env_variables(&mut self) -> Vec<String> {
        self.override_with_env_provider(|key| {
            env::var_os(key).map(|value| value.to_string_lossy().into_owned())
        })
    }

    fn override_with_env_provider<F>(&mut self, mut provider: F) -> Vec<String>
    where
        F: FnMut(&str) -> Option<String>,
    {
        let mut overridden_keys = Vec::new();

        for entry in self.values.values_mut() {
            let Some(env_value) = provider(&env_key_for_ini_key(&entry.original_key)) else {
                continue;
            };

            entry.value = env_value;
            overridden_keys.push(entry.original_key.clone());
        }

        overridden_keys
    }

    fn database_info_default(&self, name: &str, default: DatabaseInfo) -> DatabaseInfo {
        let key = format!("{name}DatabaseInfo");
        if let Some(value) = self.get(&key) {
            return parse_database_info(&key, value).unwrap_or(default);
        }

        self.legacy_database_info_default(&key, default)
    }

    fn legacy_database_info_default(&self, key: &str, default: DatabaseInfo) -> DatabaseInfo {
        let host_key = format!("{key}.Host");
        let port_key = format!("{key}.Port");
        let username_key = format!("{key}.Username");
        let password_key = format!("{key}.Password");
        let database_key = format!("{key}.Database");

        DatabaseInfo {
            host: self
                .get(&host_key)
                .map_or_else(|| default.host.clone(), ToOwned::to_owned),
            port_or_socket: self
                .get(&port_key)
                .map_or_else(|| default.port_or_socket.clone(), ToOwned::to_owned),
            username: self
                .get(&username_key)
                .map_or_else(|| default.username.clone(), ToOwned::to_owned),
            password: self
                .get(&password_key)
                .map_or_else(|| default.password.clone(), ToOwned::to_owned),
            database: self
                .get(&database_key)
                .map_or_else(|| default.database.clone(), ToOwned::to_owned),
            ssl: default.ssl,
        }
    }
}

/// Extract the actual value from the raw right-hand side of a config line.
///
/// Handles:
/// - Quoted strings: `"some value"` -> `some value` (content between quotes)
/// - Unquoted values with optional inline comments: `123 # a comment` -> `123`
fn parse_value(raw: &str) -> String {
    if raw.starts_with('"') {
        // Find the closing quote.
        if let Some(end) = raw[1..].find('"') {
            return raw[1..=end].to_string();
        }
        // No closing quote found -- treat the rest (minus the opening quote)
        // as the value, stripping an inline comment if present.
        return strip_inline_comment(&raw[1..]).to_string();
    }

    strip_inline_comment(raw).to_string()
}

/// Remove an inline `# comment` from an unquoted value and trim whitespace.
fn strip_inline_comment(s: &str) -> &str {
    match s.find('#') {
        Some(pos) => s[..pos].trim(),
        None => s.trim(),
    }
}

/// Converts an ini key to the TrinityCore `TC_*` environment variable name.
///
/// This mirrors `IniKeyToEnvVarKey` in C++
/// `/src/common/Configuration/Config.cpp`.
fn env_key_for_ini_key(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let mut result = String::from("TC_");

    for (idx, curr) in chars.iter().copied().enumerate() {
        if matches!(curr, ' ' | '.' | '-') {
            result.push('_');
            continue;
        }

        if let Some(next) = chars.get(idx + 1).copied() {
            let next_is_upper = next.is_ascii_uppercase();

            if !curr.is_ascii_uppercase() && next_is_upper {
                result.push(curr.to_ascii_uppercase());
                result.push('_');
                continue;
            }

            let curr_is_numeric = curr.is_ascii_digit();
            let next_is_numeric = next.is_ascii_digit();

            if !curr_is_numeric && next_is_numeric {
                result.push(curr.to_ascii_uppercase());
                result.push('_');
                continue;
            }

            if curr_is_numeric && !next_is_numeric {
                result.push(curr.to_ascii_uppercase());
                result.push('_');
                continue;
            }
        }

        result.push(curr.to_ascii_uppercase());
    }

    result
}

fn collect_conf_files(dir: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    if !dir.exists() || !dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut pending = vec![dir.to_path_buf()];
    let mut files = Vec::new();

    while let Some(path) = pending.pop() {
        let entries = fs::read_dir(&path)
            .map_err(|_| ConfigError::FileNotFound(path.display().to_string()))?;

        for entry in entries {
            let entry = entry.map_err(|_| ConfigError::FileNotFound(path.display().to_string()))?;
            let entry_path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|_| ConfigError::FileNotFound(entry_path.display().to_string()))?;

            if file_type.is_dir() {
                pending.push(entry_path);
            } else if file_type.is_file() && entry_path.extension().is_some_and(|ext| ext == "conf")
            {
                files.push(entry_path);
            }
        }
    }

    files.sort();
    Ok(files)
}

// ---------------------------------------------------------------------------
// Global singleton
// ---------------------------------------------------------------------------

static CONFIG: Lazy<RwLock<ConfigStore>> = Lazy::new(|| RwLock::new(ConfigStore::default()));

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load and parse a `.conf` file, replacing any previously loaded
/// configuration.
///
/// # Errors
///
/// Returns [`ConfigError::FileNotFound`] if the file cannot be read, or
/// [`ConfigError::ParseError`] if the content is malformed.
pub fn load_config(path: &str) -> Result<(), ConfigError> {
    let content =
        fs::read_to_string(path).map_err(|_| ConfigError::FileNotFound(path.to_string()))?;

    let mut store = CONFIG.write();
    store.parse(&content)
}

/// Report produced by canonical config startup loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadReport {
    /// The initial config file that was loaded.
    pub initial_file: String,
    /// Index of the successful initial file in the candidate list.
    pub candidate_index: usize,
    /// Additional `.conf` overlay files loaded from the config dir.
    pub loaded_files: Vec<String>,
    /// Keys overridden by `TC_*` environment variables.
    pub overridden_keys: Vec<String>,
}

/// Load the first readable initial config, then merge additional `.conf`
/// files from `config_dir`, then apply `TC_*` environment overrides.
///
/// This follows the C++ startup order:
/// `LoadInitial` -> `LoadAdditionalDir` -> `OverrideWithEnvVariablesIfAny`.
pub fn load_config_with_fallbacks(
    config_candidates: &[&str],
    config_dir: &str,
) -> Result<LoadReport, ConfigError> {
    let Some((candidate_index, initial_file, content)) = config_candidates
        .iter()
        .enumerate()
        .find_map(|(idx, path)| {
            fs::read_to_string(path)
                .ok()
                .map(|content| (idx, *path, content))
        })
    else {
        return Err(ConfigError::NoConfigFile(config_candidates.join(", ")));
    };

    let mut store = ConfigStore::default();
    store.parse(&content)?;

    let mut loaded_files = Vec::new();
    for path in collect_conf_files(Path::new(config_dir))? {
        let content = fs::read_to_string(&path)
            .map_err(|_| ConfigError::FileNotFound(path.display().to_string()))?;
        store.merge(&content)?;
        loaded_files.push(path.display().to_string());
    }

    let overridden_keys = store.override_with_env_variables();

    let mut global = CONFIG.write();
    *global = store;

    Ok(LoadReport {
        initial_file: initial_file.to_string(),
        candidate_index,
        loaded_files,
        overridden_keys,
    })
}

/// Retrieve a configuration value parsed as `T`.
///
/// Returns `None` when the key is absent **or** the value cannot be parsed
/// into `T`.
pub fn get_value<T: FromStr>(key: &str) -> Option<T> {
    let store = CONFIG.read();
    store.get(key).and_then(|v| v.parse::<T>().ok())
}

/// Retrieve a configuration value parsed as `T`, falling back to `default`
/// when the key is absent or unparsable.
pub fn get_value_default<T: FromStr>(key: &str, default: T) -> T {
    get_value(key).unwrap_or(default)
}

/// Retrieve a string value, returning `default` when the key is absent.
pub fn get_string_default(key: &str, default: &str) -> String {
    let store = CONFIG.read();
    store
        .get(key)
        .map(|s| s.to_string())
        .unwrap_or_else(|| default.to_string())
}

/// Parsed TrinityCore `*DatabaseInfo` semicolon connection string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseInfo {
    pub host: String,
    pub port_or_socket: String,
    pub username: String,
    pub password: String,
    pub database: String,
    pub ssl: bool,
}

impl DatabaseInfo {
    pub fn new(host: &str, port: u16, username: &str, password: &str, database: &str) -> Self {
        Self {
            host: host.to_string(),
            port_or_socket: port.to_string(),
            username: username.to_string(),
            password: password.to_string(),
            database: database.to_string(),
            ssl: false,
        }
    }
}

/// Parse a C++ TrinityCore database info value:
/// `host;port_or_socket;username;password;database[;ssl]`.
pub fn parse_database_info(key: &str, value: &str) -> Result<DatabaseInfo, ConfigError> {
    let parts: Vec<&str> = value.split(';').collect();
    if parts.len() < 5 {
        return Err(ConfigError::InvalidDatabaseInfo {
            key: key.to_string(),
            message: "expected host;port;username;password;database".to_string(),
        });
    }

    if parts[1].is_empty() {
        return Err(ConfigError::InvalidDatabaseInfo {
            key: key.to_string(),
            message: "empty port_or_socket".to_string(),
        });
    }

    Ok(DatabaseInfo {
        host: parts[0].to_string(),
        port_or_socket: parts[1].to_string(),
        username: parts[2].to_string(),
        password: parts[3].to_string(),
        database: parts[4].to_string(),
        ssl: parts
            .get(5)
            .is_some_and(|value| value.eq_ignore_ascii_case("ssl")),
    })
}

/// Read `{name}DatabaseInfo` using the C++ semicolon schema.
///
/// The legacy Rust split subkeys are accepted only as a temporary fallback for
/// the migration mini-phase tracked by `#NEXT.L0.CONFIG.REMOVE_LEGACY_DB_SUBKEYS`.
pub fn get_database_info_default(name: &str, default: DatabaseInfo) -> DatabaseInfo {
    let store = CONFIG.read();
    store.database_info_default(name, default)
}

// ---------------------------------------------------------------------------
// Internal helper for tests -- load from string instead of file
// ---------------------------------------------------------------------------

/// Load configuration from a raw string (useful for testing).
#[doc(hidden)]
pub fn load_config_from_str(content: &str) -> Result<(), ConfigError> {
    let mut store = CONFIG.write();
    store.parse(content)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    fn global_config_lock() -> MutexGuard<'static, ()> {
        TEST_LOCK.lock().expect("test lock poisoned")
    }

    /// Helper: create an isolated `ConfigStore` and parse into it so tests
    /// do not interfere with the global singleton.
    fn parse(content: &str) -> ConfigStore {
        let mut store = ConfigStore::default();
        store.parse(content).expect("parse failed");
        store
    }

    // -- Parsing basics -----------------------------------------------------

    #[test]
    fn test_basic_key_value() {
        let store = parse("WorldServerPort = 8085");
        assert_eq!(store.get("WorldServerPort"), Some("8085"));
    }

    #[test]
    fn test_quoted_string_value() {
        let store = parse(r#"DataDir = "/home/server/data""#);
        assert_eq!(store.get("DataDir"), Some("/home/server/data"));
    }

    #[test]
    fn test_quoted_string_with_spaces() {
        let store = parse(r#"Motd = "Welcome to the server!""#);
        assert_eq!(store.get("Motd"), Some("Welcome to the server!"));
    }

    #[test]
    fn test_empty_quoted_string() {
        let store = parse(r#"Empty = """#);
        assert_eq!(store.get("Empty"), Some(""));
    }

    // -- Comments -----------------------------------------------------------

    #[test]
    fn test_full_line_comment_ignored() {
        let store = parse("# this is a comment\nPort = 3724");
        assert_eq!(store.get("Port"), Some("3724"));
        // Only one entry in the map.
        assert_eq!(store.values.len(), 1);
    }

    #[test]
    fn test_inline_comment_stripped() {
        let store = parse("Port = 3724 # default bnet port");
        assert_eq!(store.get("Port"), Some("3724"));
    }

    #[test]
    fn test_inline_comment_with_quoted_value() {
        // The inline comment is outside the quotes, so the value is just
        // the quoted content.
        let store = parse(r#"DataDir = "/data" # path to data"#);
        assert_eq!(store.get("DataDir"), Some("/data"));
    }

    // -- Empty / whitespace lines -------------------------------------------

    #[test]
    fn test_empty_lines_ignored() {
        let content = "\n\n  \nKey = val\n\n";
        let store = parse(content);
        assert_eq!(store.get("Key"), Some("val"));
        assert_eq!(store.values.len(), 1);
    }

    // -- Case-insensitive lookup --------------------------------------------

    #[test]
    fn test_case_insensitive_lookup() {
        let store = parse("DataDir = /data");
        assert_eq!(store.get("datadir"), Some("/data"));
        assert_eq!(store.get("DATADIR"), Some("/data"));
        assert_eq!(store.get("DataDir"), Some("/data"));
    }

    // -- Numeric parsing ----------------------------------------------------

    #[test]
    fn test_parse_integer() {
        let store = parse("Port = 8085");
        let val: u16 = store.get("Port").unwrap().parse().unwrap();
        assert_eq!(val, 8085);
    }

    #[test]
    fn test_parse_float() {
        let store = parse("Rate.XP.Kill = 1.5");
        let val: f64 = store.get("Rate.XP.Kill").unwrap().parse().unwrap();
        assert!((val - 1.5).abs() < f64::EPSILON);
    }

    // -- Defaults -----------------------------------------------------------

    #[test]
    fn test_get_value_default_missing_key() {
        // Use the global API with a key we know does not exist.
        let val: i32 = get_value_default("__nonexistent_key_42__", 99);
        assert_eq!(val, 99);
    }

    #[test]
    fn test_get_string_default_missing_key() {
        let val = get_string_default("__nonexistent_key_43__", "fallback");
        assert_eq!(val, "fallback");
    }

    // -- Global API round-trip ----------------------------------------------

    #[test]
    fn test_global_load_and_get() {
        let _guard = global_config_lock();
        load_config_from_str("TestKey = 42\nGreeting = \"hello world\"").expect("load failed");

        assert_eq!(get_value::<i32>("TestKey"), Some(42));
        assert_eq!(get_value::<String>("Greeting"), Some("hello world".into()));
        assert_eq!(get_string_default("Greeting", ""), "hello world");
    }

    // -- Error paths --------------------------------------------------------

    #[test]
    fn test_file_not_found() {
        let err = load_config("/tmp/__does_not_exist_12345__.conf");
        assert!(err.is_err());
        match err.unwrap_err() {
            ConfigError::FileNotFound(p) => {
                assert!(p.contains("__does_not_exist_12345__"));
            }
            other => panic!("expected FileNotFound, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_error_no_equals() {
        let mut store = ConfigStore::default();
        let result = store.parse("this line has no equals sign");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ParseError { line, .. } => assert_eq!(line, 1),
            other => panic!("expected ParseError, got: {other:?}"),
        }
    }

    #[test]
    fn test_parse_error_empty_key() {
        let mut store = ConfigStore::default();
        let result = store.parse(" = value");
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::ParseError { line, message } => {
                assert_eq!(line, 1);
                assert!(message.contains("empty key"));
            }
            other => panic!("expected ParseError, got: {other:?}"),
        }
    }

    // -- Multiple keys ------------------------------------------------------

    #[test]
    fn test_multiple_keys() {
        let content = r#"
# Server settings
WorldServerPort = 8085
DataDir = "/opt/wow/data"
Rate.XP.Kill = 2.0
LogLevel = 3
"#;
        let store = parse(content);
        assert_eq!(store.get("WorldServerPort"), Some("8085"));
        assert_eq!(store.get("DataDir"), Some("/opt/wow/data"));
        assert_eq!(store.get("Rate.XP.Kill"), Some("2.0"));
        assert_eq!(store.get("LogLevel"), Some("3"));
    }

    // -- Overwrite on reload ------------------------------------------------

    #[test]
    fn test_reload_replaces_values() {
        let mut store = ConfigStore::default();
        store.parse("Key = old").unwrap();
        assert_eq!(store.get("Key"), Some("old"));

        store.parse("Key = new").unwrap();
        assert_eq!(store.get("Key"), Some("new"));
    }

    // -- Value with equals sign ---------------------------------------------

    #[test]
    fn test_value_containing_equals() {
        let store = parse(r#"ConnString = "server=localhost;port=3306""#);
        assert_eq!(store.get("ConnString"), Some("server=localhost;port=3306"));
    }

    // -- Quoted value containing hash ---------------------------------------

    #[test]
    fn test_quoted_value_with_hash() {
        let store = parse(r##"Color = "#FF0000""##);
        assert_eq!(store.get("Color"), Some("#FF0000"));
    }

    #[test]
    fn test_database_info_semicolon_parser() {
        let info = parse_database_info(
            "LoginDatabaseInfo",
            "127.0.0.1;3306;trinity;trinity;auth;ssl",
        )
        .expect("db info should parse");

        assert_eq!(info.host, "127.0.0.1");
        assert_eq!(info.port_or_socket, "3306");
        assert_eq!(info.username, "trinity");
        assert_eq!(info.password, "trinity");
        assert_eq!(info.database, "auth");
        assert!(info.ssl);
    }

    #[test]
    fn test_database_info_accepts_unix_socket_like_cpp() {
        let info = parse_database_info(
            "WorldDatabaseInfo",
            ".;/var/run/mysqld/mysqld.sock;trinity;trinity;world",
        )
        .expect("db info should parse");

        assert_eq!(info.host, ".");
        assert_eq!(info.port_or_socket, "/var/run/mysqld/mysqld.sock");
        assert_eq!(info.database, "world");
    }

    #[test]
    fn test_get_database_info_uses_canonical_key_before_legacy_split_keys() {
        let store = parse(
            r#"
LoginDatabaseInfo = "127.0.0.1;3306;trinity;trinity;auth"
LoginDatabaseInfo.Host = "legacy"
"#,
        );

        let info = store.database_info_default(
            "Login",
            DatabaseInfo::new("fallback", 1, "fallback", "fallback", "fallback"),
        );

        assert_eq!(info.host, "127.0.0.1");
        assert_eq!(info.port_or_socket, "3306");
        assert_eq!(info.username, "trinity");
        assert_eq!(info.password, "trinity");
        assert_eq!(info.database, "auth");
    }

    #[test]
    fn test_get_database_info_legacy_split_fallback_is_temporary() {
        let store = parse(
            r#"
LoginDatabaseInfo.Host = "127.0.0.2"
LoginDatabaseInfo.Port = 3307
LoginDatabaseInfo.Username = "legacy_user"
LoginDatabaseInfo.Password = "legacy_pass"
LoginDatabaseInfo.Database = "legacy_auth"
"#,
        );

        let info = store.database_info_default(
            "Login",
            DatabaseInfo::new("fallback", 1, "fallback", "fallback", "fallback"),
        );

        assert_eq!(info.host, "127.0.0.2");
        assert_eq!(info.port_or_socket, "3307");
        assert_eq!(info.username, "legacy_user");
        assert_eq!(info.password, "legacy_pass");
        assert_eq!(info.database, "legacy_auth");
    }

    #[test]
    fn test_env_key_for_ini_key_matches_trinity_examples() {
        assert_eq!(env_key_for_ini_key("SomeConfig"), "TC_SOME_CONFIG");
        assert_eq!(
            env_key_for_ini_key("myNestedConfig.opt1"),
            "TC_MY_NESTED_CONFIG_OPT_1"
        );
        assert_eq!(
            env_key_for_ini_key("LogDB.Opt.ClearTime"),
            "TC_LOG_DB_OPT_CLEAR_TIME"
        );
    }

    #[test]
    fn test_env_override_provider_overrides_scalar_keys() {
        let mut store = parse("WorldServerPort = 8085\n");
        let overridden = store.override_with_env_provider(|key| {
            (key == "TC_WORLD_SERVER_PORT").then(|| "9100".to_string())
        });

        assert_eq!(overridden, vec!["WorldServerPort"]);
        assert_eq!(store.get("WorldServerPort"), Some("9100"));
    }

    #[test]
    fn test_load_config_with_fallbacks_loads_dir_overlays() {
        let _guard = global_config_lock();
        let root = unique_temp_dir("load_config_with_fallbacks");
        let conf_dir = root.join("worldserver.conf.d");
        fs::create_dir_all(conf_dir.join("nested")).expect("mkdir failed");

        let primary = root.join("worldserver.conf");
        let overlay = conf_dir.join("nested").join("override.conf");
        let ignored = conf_dir.join("ignored.txt");

        fs::write(
            &primary,
            r#"
WorldServerPort = 8085
LoginDatabaseInfo = "127.0.0.1;3306;trinity;trinity;auth"
"#,
        )
        .expect("write primary failed");
        fs::write(&overlay, "WorldServerPort = 9000\n").expect("write overlay failed");
        fs::write(&ignored, "WorldServerPort = 1\n").expect("write ignored failed");

        let report = load_config_with_fallbacks(
            &[primary.to_str().expect("utf8 path")],
            conf_dir.to_str().expect("utf8 path"),
        )
        .expect("config should load");

        assert_eq!(report.candidate_index, 0);
        assert_eq!(report.loaded_files.len(), 1);
        assert_eq!(get_value::<u16>("WorldServerPort"), Some(9000));

        let info = get_database_info_default(
            "Login",
            DatabaseInfo::new("fallback", 1, "fallback", "fallback", "fallback"),
        );
        assert_eq!(info.database, "auth");

        fs::remove_dir_all(root).expect("cleanup failed");
    }

    #[test]
    fn test_load_config_with_fallbacks_uses_lowercase_before_legacy_name() {
        let _guard = global_config_lock();
        let root = unique_temp_dir("load_config_candidate_order");
        let lower = root.join("bnetserver.conf");
        let legacy = root.join("BNetServer.conf");

        fs::write(&lower, "BattlenetPort = 1119\n").expect("write lower failed");
        fs::write(&legacy, "BattlenetPort = 2222\n").expect("write legacy failed");

        let report = load_config_with_fallbacks(
            &[
                lower.to_str().expect("utf8 path"),
                legacy.to_str().expect("utf8 path"),
            ],
            root.join("bnetserver.conf.d").to_str().expect("utf8 path"),
        )
        .expect("config should load");

        assert_eq!(report.candidate_index, 0);
        assert_eq!(get_value::<u16>("BattlenetPort"), Some(1119));

        fs::remove_dir_all(root).expect("cleanup failed");
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let mut path = env::temp_dir();
        path.push(format!(
            "rustycore_wow_config_{name}_{}_{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));

        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("temp dir failed");
        path
    }
}
