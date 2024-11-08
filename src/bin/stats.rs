use std::time::Duration;

pub struct Stats {
    stamp: std::time::Instant,
    last_frametimes: std::collections::VecDeque<Duration>,
    frames: u64,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            stamp: std::time::Instant::now(),
            last_frametimes: {
                let mut dq = std::collections::VecDeque::new();
                for _ in 0..30 {
                    dq.push_back(Duration::default());
                }
                dq
            },
            frames: 0,
        }
    }

    pub fn capture_frame(&mut self) {
        self.last_frametimes.pop_front();
        self.last_frametimes.push_back(self.stamp.elapsed());
        self.frames += 1;
        self.stamp = std::time::Instant::now();
    }

    pub fn get_average(&self) -> Duration {
        self.last_frametimes.iter().sum::<Duration>() / 30
    }
}
