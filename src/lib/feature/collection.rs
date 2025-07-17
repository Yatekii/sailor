use crate::{
    config::MAX_FEATURES,
    css::{RulesCache, Selector},
};

use super::{Feature, FeatureStyle};

#[derive(Debug, Clone)]
pub struct FeatureCollection {
    features: Vec<Feature>,
    layers: Vec<LayerInfo>,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub id: usize,
    pub name: String,
    pub display: bool,
}

impl FeatureCollection {
    pub fn new() -> Self {
        Self {
            features: Vec::with_capacity(MAX_FEATURES),
            layers: {
                let mut layers = Vec::with_capacity(30);
                layers.push(LayerInfo {
                    id: 0,
                    name: "background".to_string(),
                    display: true,
                });
                layers
            },
        }
    }

    pub fn features(&self) -> &Vec<Feature> {
        &self.features
    }

    pub fn features_mut(&mut self) -> &mut Vec<Feature> {
        &mut self.features
    }

    pub fn layers(&self) -> &Vec<LayerInfo> {
        &self.layers
    }

    pub fn layers_mut(&mut self) -> &mut Vec<LayerInfo> {
        &mut self.layers
    }

    fn get_feature_id(&mut self, selector: &crate::css::Selector) -> Option<u32> {
        self.features
            .iter_mut()
            .enumerate()
            .find(|(_, feature)| &feature.selector == selector)
            .map(|(i, _)| i as u32)
    }

    fn add_feature(&mut self, mut feature: Feature) -> u32 {
        assert!(self.features.len() < MAX_FEATURES);
        feature.id = self.features.len() as u32;
        self.features.push(feature);
        self.features.len() as u32 - 1
    }

    pub fn ensure_feature(&mut self, selector: &Selector, layer_id: usize) -> u32 {
        if let Some(feature_id) = self.get_feature_id(selector) {
            feature_id
        } else {
            self.add_feature(Feature::new(selector.clone(), layer_id))
        }
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
            feature.load_style(
                zoom,
                css_cache,
                self.layers
                    .iter()
                    .find(|l| l.id == feature.layer_id)
                    .map(|l| l.display)
                    .unwrap_or_default(),
            )
        }
    }

    pub fn assemble_style_buffer(&self) -> Vec<FeatureStyle> {
        self.features.iter().map(|f| f.style).collect()
    }
}

impl Default for FeatureCollection {
    fn default() -> Self {
        Self::new()
    }
}
