lazy_static! {
    /// This is an example for using doc comment attributes
    pub static ref CONFIG: Config = Config::new().expect("Config could not be loaded.");
}

#[derive(Debug, Deserialize)]
pub struct Renderer {
    pub vertex_shader: String,
    pub fragment_shader: String,
    pub css: String,
    pub max_tiles: usize,
    pub max_features: u64,
    pub tile_size: usize,
    pub msaa_samples: u32,
}

#[derive(Debug, Deserialize)]
pub struct General {
    pub log_level: log::Level,
    pub display_framerate: bool,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub general: General,
    pub renderer: Renderer,
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let mut s = config::Config::new();

        // Start off by merging in the "default" configuration file
        s.merge(config::File::with_name("config/default"))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        s.merge(config::File::with_name("config/local").required(false))?;

        // You can deserialize (and thus freeze) the entire configuration as
        s.try_into()
    }
}
