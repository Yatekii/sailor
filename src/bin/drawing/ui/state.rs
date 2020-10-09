pub struct UIState {
    pub loaction_finder: LocationFinderState,
}

impl UIState {
    pub fn new() -> Self {
        Self {
            loaction_finder: LocationFinderState::new(),
        }
    }
}

pub struct LocationFinderState {
    pub input: String,
}

impl LocationFinderState {
    pub fn new() -> Self {
        Self {
            input: String::new(),
        }
    }
}
