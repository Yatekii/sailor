use once_cell::sync::Lazy;
use serde::Deserialize;

pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::new().expect("Config could not be loaded."));

#[derive(Debug, Deserialize)]
pub struct Renderer {
    pub vertex_shader: String,
    pub fragment_shader: String,
    pub css: String,
    pub max_tiles: usize,
    pub max_features: u64,
    pub tile_size: u32,
    pub msaa_samples: u32,
    pub selection_tags: Vec<String>,
    pub temperature: Temperature,
}

#[derive(Debug, Deserialize)]
pub struct Temperature {
    pub vertex_shader: String,
    pub fragment_shader: String,
}

#[derive(Debug, Deserialize)]
pub struct General {
    pub log_level: log::Level,
    pub display_framerate: bool,
    pub data_root: String,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub general: General,
    pub renderer: Renderer,
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let config = config::Config::builder()
            .add_source(config::File::with_name("config/default"))
            .add_source(config::File::with_name("config/local").required(false))
            .build()?;

        config.try_deserialize()
    }
}
