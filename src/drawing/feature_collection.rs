use crate::css::RulesCache;
use crate::drawing::feature::{
    Feature,
    FeatureStyle,
};

#[derive(Debug, Clone)]
pub struct FeatureCollection {
    features: Vec<Feature>,
    n_features_max: u32,
}

impl FeatureCollection {
    pub fn new(n_features_max: u32) -> Self {
        Self {
            features: vec![],
            n_features_max,
        }
    }

    pub fn get_features(&self) -> &Vec<Feature> {
        &self.features
    }

    pub fn get_feature_id(&mut self, selector: &crate::css::Selector) -> Option<u32> {
        self.features.iter_mut().enumerate().find(|(_, feature)| &feature.selector == selector).map(|(i, _)| i as u32)
    }

    pub fn add_feature(&mut self, mut feature: Feature) -> u32 {
        assert!(self.features.len() < self.n_features_max as usize);
        feature.id = self.features.len() as u32;
        self.features.push(feature);
        self.features.len() as u32 - 1
    }

    pub fn is_visible(&self, feature_id: u32) -> bool {
        let feature = &self.features[feature_id as usize];
        let bga = feature.style.background_color.a;
        let bca = feature.style.border_color.a;
        !(!feature.style.display || (bga == 0.0 && bca == 0.0))
    }

    pub fn has_alpha(&self, feature_id: u32) -> bool {
        let feature = &self.features[feature_id as usize];
        let bga = feature.style.background_color.a;
        let bca = feature.style.border_color.a;
        bga < 1.0 || bca < 1.0
    }

    pub fn has_outline(&self, feature_id: u32) -> bool {
        let feature = &self.features[feature_id as usize];
        let bw = feature.style.background_color.a;
        let bca = feature.style.border_color.a;
        bw > 0.0 && bca > 0.0
    }

    pub fn get_zindex(&self, feature_id: u32) -> f32 {
        self.features[feature_id as usize].style.z_index
    }

    pub fn load_styles(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        for feature in &mut self.features {
            feature.load_style(zoom, css_cache)
        }
    }

    pub fn assemble_style_buffer(&self) -> Vec<FeatureStyle> {
        self.features.iter().map(|f| f.style).collect()
    }
}