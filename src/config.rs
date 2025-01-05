use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

/// The structure of the configuration file.
#[derive(Deserialize, Clone, Debug)]
pub(crate) struct Config {
    pub(crate) prometheus_listen_address: String,
    pub(crate) http_listen_address: String,
    pub(crate) amqp_server_address: String,
    pub(crate) submarine_http_url: String,
    pub(crate) fixtures_path: String,
}

impl Config {
    /// Reads a config from a file.
    pub(crate) fn read_from_file<P: AsRef<Path>>(path: P) -> Result<Config> {
        let contents = fs::read(path).context("unable to read file")?;

        let cfg: Config =
            serde_yaml::from_slice(contents.as_slice()).context("unable to parse config")?;

        Ok(cfg)
    }
}
