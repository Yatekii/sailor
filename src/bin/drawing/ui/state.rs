use crate::drawing::layer::WindControls;

pub struct UIState {
    pub loaction_finder: LocationFinderState,
    pub wind: WindControls,
}

impl UIState {
    pub fn new() -> Self {
        Self {
            loaction_finder: LocationFinderState::new(),
            wind: WindControls::default(),
        }
    }
}

pub struct LocationFinderState {
    pub input: String,
}

impl LocationFinderState {
    pub fn new() -> Self {
        Self {
            input: "47.3769 8.5417".into(),
        }
    }
}
