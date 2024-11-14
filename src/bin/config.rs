use once_cell::sync::Lazy;
use serde::Deserialize;

pub static CONFIG: Lazy<Config> = Lazy::new(|| Config::new().expect("Config could not be loaded."));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub general: General,
    pub map: MapState,
    pub renderer: Renderer,
    pub window: Window,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Window {
    pub size: WindowSize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WindowSize {
    Windowed { width: f64, height: f64 },
    Fullscreen,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
#[serde(rename_all = "kebab-case")]
pub struct Temperature {
    pub vertex_shader: String,
    pub fragment_shader: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct General {
    pub log: Log,
    pub display_framerate: bool,
    pub data_root: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Log {
    pub level: log::Level,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MapState {
    pub initial: InitialMapState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InitialMapState {
    pub zoom: f32,
    pub center: InitialCenterPoint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InitialCenterPoint {
    pub latitude: f32,
    pub longitude: f32,
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
