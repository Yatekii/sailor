pub struct Stats {
    stamp: std::time::Instant,
    last_frametimes: std::collections::VecDeque<u64>,
    frames: u64,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            stamp: std::time::Instant::now(),
            last_frametimes: {
                let mut dq = std::collections::VecDeque::new();
                for i in 0..30 {
                    dq.push_back(i);
                }
                dq
            },
            frames: 0,
        }
    }

    pub fn capture_frame(&mut self) {
        self.last_frametimes.pop_front();
        self.last_frametimes
            .push_back(self.stamp.elapsed().as_micros() as u64);
        self.frames += 1;
        self.stamp = std::time::Instant::now();
    }

    pub fn get_average(&self) -> f64 {
        self.last_frametimes.iter().sum::<u64>() as f64 / 30.0
    }

    pub fn get_last_delta(&self) -> f32 {
        (self.last_frametimes[29] as f64 / 1_000_000f64) as f32
    }
}
