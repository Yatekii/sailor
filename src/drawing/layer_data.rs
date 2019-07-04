use std::slice::Iter;
use crate::css::RulesCache;
use crate::drawing::feature::{
    Feature,
    FeatureStyle,
};

#[derive(Debug, Clone)]
pub struct LayerData {
    pub name: String,
    pub id: u32,
    pub features: Vec<Feature>,
}

impl LayerData {
    pub fn new(name: impl Into<String>, id: u32, n: u32) -> Self {
        Self {
            name: name.into(),
            id,
            features: vec![],
        }
    }

    pub fn get_feature_mut(&mut self, selector: &crate::css::Selector) -> Option<&mut Feature> {
        self.features.iter_mut().find(|feature| &feature.selector == selector)
    }

    pub fn get_feature_id(&mut self, selector: &crate::css::Selector) -> Option<u32> {
        self.features.iter_mut().enumerate().find(|(i, feature)| &feature.selector == selector).map(|(i, _)| i as u32)
    }

    pub fn add_feature(&mut self, feature: Feature) -> u32 {
        self.features.push(feature);
        self.features.len() as u32
    }

    pub fn has_outline(&self) -> bool {
        for feature in &self.features {
            if feature.style.border_width > 0.0 && feature.style.border_color.a > 0.0 {
                return true;
            }
        }
        false
    }

    pub fn has_fill(&self) -> bool {
        for feature in &self.features {
            if feature.style.background_color.a > 0.0 {
                return true;
            }
        }
        false
    }

    pub fn assemble_style_buffer(&self, buffer: &mut Vec<FeatureStyle>) -> u32 {
        for feature in &self.features {
            buffer.push(feature.style.clone());
        }
        self.features.len() as u32
    }
}

pub struct LayerCollection {
    layer_datas: Vec<Option<LayerData>>,
    n_features_max: u32,
}

impl LayerCollection {
    pub fn new(n_layers: u32, n_features_max: u32) -> Self {
        Self {
            layer_datas: vec![None; n_layers as usize],
            n_features_max,
        }
    }

    pub fn get_layer_mut(&mut self, id: u32) -> Option<&mut LayerData> {
        self.layer_datas[id as usize].as_mut()
    }

    pub fn get_layer(&self, id: u32) -> Option<&LayerData> {
        self.layer_datas[id as usize].as_ref()
    }

    pub fn set_layer(&mut self, id: u32, layer: LayerData) {
        self.layer_datas[id as usize] = Some(layer);
    }

    pub fn create_new_layer(&mut self, id: u32, name: impl Into<String>) -> &mut LayerData {
        self.layer_datas[id as usize] = Some(LayerData::new(name, id, 20));
        self.layer_datas[id as usize].as_mut().unwrap()
    }

    pub fn iter(&self) -> Iter<'_, Option<LayerData>> {
        self.layer_datas.iter()
    }

    pub fn load_styles(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        for layer_data in &mut self.layer_datas {
            if let Some(layer_data) = layer_data {
                for feature in &mut layer_data.features {
                    feature.load_style(zoom, css_cache)
                }
            }
        }
    }

    pub fn assemble_style_buffer(&self) -> Vec<FeatureStyle> {
        let mut buffer = Vec::with_capacity(self.layer_datas.len() * self.n_features_max as usize);

        for layer_data in &self.layer_datas {
            if let Some(layer_data) = layer_data {
                let len = layer_data.assemble_style_buffer(&mut buffer);
                for _ in len..self.n_features_max {
                    buffer.push(Default::default());
                }
            } else {
                for _ in 0..self.n_features_max {
                    buffer.push(Default::default());
                }
            }
        }

        buffer
    }

    pub fn get_sizes(&self) -> (u32, u32) {
        (self.layer_datas.len() as u32, self.n_features_max)
    }
}