use config::{Config, ConfigError, Environment, File};
use derive_more::Display;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Server {
    pub port: u16,
    pub ip: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Enclave {
    pub publickeypath: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Secp {
    pub privatekeypath: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Configuration {
    pub server: Server,
    pub enclave: Enclave,
    pub secp: Secp,
    pub env: ENV,
}
impl Configuration {
    pub fn new() -> Result<Self, ConfigError> {
        let env = std::env::var("RUN_ENV").unwrap_or_else(|_| "development".into());
        let builder = Config::builder()
            .set_default("env", env.clone())?
            .add_source(File::with_name(CONFIG_FILE_PATH))
            .add_source(File::with_name(&format!("{}{}", CONFIG_FILE_PREFIX, env)))
            .add_source(Environment::with_prefix("bi").separator("_"));

        let config = builder
            .build()
            .map_err(|_| ConfigError::Message("config builder error".to_string()))?;
        Ok(config.try_deserialize()?)
    }
}
#[derive(Clone, Debug, Deserialize, Display)]
#[serde(rename_all = "lowercase")]
pub enum ENV {
    Development,
    Production,
}

impl From<&str> for ENV {
    fn from(env: &str) -> Self {
        match env {
            "production" => ENV::Development,
            _ => ENV::Production,
        }
    }
}

const CONFIG_FILE_PATH: &str = "src/config/default";
const CONFIG_FILE_PREFIX: &str = "src/config/";
