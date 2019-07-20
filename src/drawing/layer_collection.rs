use std::slice::Iter;
use crate::css::RulesCache;
use crate::drawing::feature::{
    Feature,
    FeatureStyle,
};

#[derive(Debug, Clone)]
pub struct LayerCollection {
    layers: Vec<bool>,
    features: Vec<Feature>,
    n_features_max: u32,
}

impl LayerCollection {
    pub fn new(n_layers: u32, n_features_max: u32) -> Self {
        Self {
            layers: vec![false; n_layers as usize],
            features: vec![],
            n_features_max,
        }
    }

    pub fn is_layer_set(&self, id: u32) -> bool {
        self.layers[id as usize]
    }

    pub fn set_layer(&mut self, id: u32) {
        self.layers[id as usize] = true;
    }

    pub fn iter_layers(&self) -> Iter<'_, bool> {
        self.layers.iter()
    }

    pub fn get_feature_id(&mut self, selector: &crate::css::Selector) -> Option<u32> {
        self.features.iter_mut().enumerate().find(|(_, feature)| &feature.selector == selector).map(|(i, _)| i as u32)
    }

    pub fn add_feature(&mut self, feature: Feature) -> u32 {
        assert!(self.features.len() < self.n_features_max as usize);
        self.features.push(feature);
        self.features.len() as u32 - 1
    }

    pub fn _has_outline(&self) -> bool {
        for feature in &self.features {
            if feature.style.border_width > 0.0 && feature.style.border_color.a > 0.0 {
                return true;
            }
        }
        false
    }

    pub fn _has_fill(&self) -> bool {
        for feature in &self.features {
            if feature.style.background_color.a > 0.0 {
                return true;
            }
        }
        false
    }

    pub fn is_visible(&self, feature_id: u32) -> bool {
        self.features[feature_id as usize].style.display
    }

    pub fn has_outline(&self, feature_id: u32) -> bool {
           self.features[feature_id as usize].style.border_width > 0.0
        && self.features[feature_id as usize].style.border_color.a > 0.0
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