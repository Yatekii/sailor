use std::collections::VecDeque;
use std::time::Duration;

pub const FPS_SAMPLES: usize = 3000;
/// Rolling time window for the profiling histograms. Frame-count windows drift
/// with framerate (~3s at 350fps vs ~50s at 60fps); a time window is stable.
const WINDOW_SECS: f64 = 10.0;

pub struct Stats {
    stamp: web_time::Instant,
    /// Monotonic clock start (never reset, unlike `stamp`) for sample timestamps.
    epoch: web_time::Instant,
    last_frametimes: VecDeque<Duration>,
    frames: u64,
    // Named timing series (cpu spans, gpu passes). Insertion-ordered.
    series: Vec<Series>,
}

pub struct Series {
    pub name: &'static str,
    /// (timestamp_secs, duration) samples within the last `WINDOW_SECS`.
    samples: VecDeque<(f64, Duration)>,
}

impl Series {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            samples: VecDeque::new(),
        }
    }

    fn push(&mut self, now: f64, d: Duration) {
        self.samples.push_back((now, d));
        while let Some(&(t, _)) = self.samples.front() {
            if now - t > WINDOW_SECS {
                self.samples.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    pub fn durations(&self) -> impl Iterator<Item = Duration> + '_ {
        self.samples.iter().map(|(_, d)| *d)
    }

    pub fn average(&self) -> Duration {
        if self.samples.is_empty() {
            return Duration::ZERO;
        }
        self.durations().sum::<Duration>() / self.samples.len() as u32
    }

    pub fn max(&self) -> Duration {
        self.durations().max().unwrap_or(Duration::ZERO)
    }
}

impl Stats {
    pub fn new() -> Self {
        Self {
            stamp: web_time::Instant::now(),
            epoch: web_time::Instant::now(),
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

    /// Mean frametime over roughly the last second — sum the most recent frames
    /// until ~1s of them accumulates, so the readout tracks the current rate
    /// instead of a multi-second average. The full buffer still feeds the sparkline.
    pub fn get_average(&self) -> Duration {
        let mut total = Duration::ZERO;
        let mut n = 0u32;
        for d in self.last_frametimes.iter().rev() {
            total += *d;
            n += 1;
            if total >= Duration::from_secs(1) {
                break;
            }
        }
        if n == 0 { Duration::ZERO } else { total / n }
    }

    pub fn get_times(&mut self) -> impl Iterator<Item = &Duration> {
        self.last_frametimes.iter()
    }

    /// Record one sample for a named timing series, creating it on first sight.
    pub fn record(&mut self, name: &'static str, d: Duration) {
        let now = self.epoch.elapsed().as_secs_f64();
        if let Some(s) = self.series.iter_mut().find(|s| s.name == name) {
            s.push(now, d);
        } else {
            let mut s = Series::new(name);
            s.push(now, d);
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
