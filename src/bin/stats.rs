use std::collections::VecDeque;
use std::time::Duration;

pub const FPS_SAMPLES: usize = 3000;
// Window per profiling series, used to build the histograms.
pub const HIST_SAMPLES: usize = 1024;

pub struct Stats {
    stamp: web_time::Instant,
    last_frametimes: VecDeque<Duration>,
    frames: u64,
    // Named timing series (cpu spans, gpu passes). Insertion-ordered.
    series: Vec<Series>,
}

pub struct Series {
    pub name: &'static str,
    pub samples: VecDeque<Duration>,
}

impl Series {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: VecDeque::with_capacity(HIST_SAMPLES),
        }
    }

    fn push(&mut self, d: Duration) {
        if self.samples.len() == HIST_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(d);
    }

    pub fn average(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        self.samples.iter().sum::<Duration>() / self.samples.len() as u32
    }

    pub fn max(&self) -> Duration {
        self.samples.iter().copied().max().unwrap_or(Duration::ZERO)
    }
}

impl Stats {
    pub fn new() -> Self {
        Self {
            stamp: web_time::Instant::now(),
            last_frametimes: {
                let mut dq = VecDeque::new();
                for _ in 0..FPS_SAMPLES {
                    dq.push_back(Duration::from_millis(2));
                }
                dq
            },
            frames: 0,
            series: Vec::new(),
        }
    }

    pub fn capture_frame(&mut self) {
        self.last_frametimes.pop_front();
        self.last_frametimes.push_back(self.stamp.elapsed());
        self.frames += 1;
        self.stamp = web_time::Instant::now();
    }

    pub fn get_average(&self) -> Duration {
        self.last_frametimes.iter().sum::<Duration>() / FPS_SAMPLES as u32
    }

    pub fn get_times(&mut self) -> impl Iterator<Item = &Duration> {
        self.last_frametimes.iter()
    }

    /// Record one sample for a named timing series, creating it on first sight.
    pub fn record(&mut self, name: &'static str, d: Duration) {
        if let Some(s) = self.series.iter_mut().find(|s| s.name == name) {
            s.push(d);
        } else {
            let mut s = Series::new(name);
            s.push(d);
            self.series.push(s);
        }
    }

    pub fn series(&self) -> &[Series] {
        &self.series
    }
}

impl crate::drawing::layer::StatSink for Stats {
    fn record(&mut self, name: &'static str, dur: Duration) {
        Stats::record(self, name, dur);
    }
}
