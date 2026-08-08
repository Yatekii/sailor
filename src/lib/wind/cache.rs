use std::path::Path;
use std::time::Duration;

use web_time::Instant;

use crate::platform::{Task, spawn_task};
use crate::wind::{WindField, WindModel};
use sailor_platform::wind_fetch::fetch_wind_json;

/// Geographic bounding box in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bbox {
    pub min_lon: f32,
    pub min_lat: f32,
    pub max_lon: f32,
    pub max_lat: f32,
}

impl Bbox {
    pub fn width(&self) -> f32 {
        (self.max_lon - self.min_lon).abs()
    }

    pub fn height(&self) -> f32 {
        (self.max_lat - self.min_lat).abs()
    }

    /// Clamp to the valid web-mercator range. At low zoom the viewport spills
    /// past the world; without this the fetch asks for out-of-range lat/lon,
    /// open-meteo rejects it, and a stale patch is left frozen on screen.
    pub fn clamp_valid(&self) -> Bbox {
        Bbox {
            min_lon: self.min_lon.clamp(-180.0, 180.0),
            min_lat: self.min_lat.clamp(-85.0, 85.0),
            max_lon: self.max_lon.clamp(-180.0, 180.0),
            max_lat: self.max_lat.clamp(-85.0, 85.0),
        }
    }

    /// Expand outward to the nearest `step`-degree lattice lines, so sample
    /// points stay pinned to a fixed global grid (multiples of `step`) instead
    /// of sliding as you pan, and the fetch only changes when you cross a line.
    pub fn snap(&self, step: f32) -> Bbox {
        Bbox {
            min_lon: (self.min_lon / step).floor() * step,
            min_lat: (self.min_lat / step).floor() * step,
            max_lon: (self.max_lon / step).ceil() * step,
            max_lat: (self.max_lat / step).ceil() * step,
        }
    }
}

/// Safety cap on lattice points per request; bounds the url length, the instance
/// count, and the open-meteo quota cost (billed per location). The step is
/// doubled until the lattice fits.
const MAX_POINTS: usize = 350;
/// Let the view settle this long before fetching, so panning across many
/// lattice cells fires one request instead of a burst.
const DEBOUNCE: Duration = Duration::from_millis(400);
/// After a failed fetch, wait this long before retrying, so a transient error
/// (a 429, a dropped connection) self-heals instead of freezing the field —
/// without hammering the api at frame rate.
const BACKOFF: Duration = Duration::from_secs(15);

/// Pick a "nice" lattice step (degrees) for a viewport `span` degrees wide, so
/// the overlay keeps a roughly constant on-screen arrow density: `target` arrows
/// across the span. Never finer than `min_step` (the model's native grid —
/// finer only returns interpolated duplicates). Coarse when zoomed out, fine in.
pub fn nice_step(span: f32, target: f32, min_step: f32) -> f32 {
    const STEPS: [f32; 13] = [
        0.01, 0.02, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0,
    ];
    let ideal = (span / target.max(1.0)).max(min_step);
    for s in STEPS {
        if s >= ideal {
            return s;
        }
    }
    30.0
}

/// Lattice point count for a snapped bbox at `step`.
fn point_count(bbox: Bbox, step: f32) -> usize {
    let cols = (bbox.width() / step).round() as usize + 1;
    let rows = (bbox.height() / step).round() as usize + 1;
    cols * rows
}

/// Build the Open-Meteo request for wind on a fixed global lattice of `step`
/// degrees covering `bbox` (assumed already snapped + clamped), plus a stable
/// disk-cache key. Points sit at multiples of `step`, so they stay pinned to
/// the same spots as you pan. Requests the current step, wind in knots.
pub fn open_meteo_url(model: &str, bbox: Bbox, step: f32) -> (String, String) {
    let cols = ((bbox.width() / step).round() as i32).max(1);
    let rows = ((bbox.height() / step).round() as i32).max(1);
    let mut lats = Vec::new();
    let mut lons = Vec::new();
    for r in 0..=rows {
        let lat = (bbox.min_lat + r as f32 * step).clamp(-85.0, 85.0);
        for c in 0..=cols {
            let lon = (bbox.min_lon + c as f32 * step).clamp(-180.0, 180.0);
            lats.push(format!("{lat:.4}"));
            lons.push(format!("{lon:.4}"));
        }
    }
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}\
         &current=wind_speed_10m,wind_direction_10m&wind_speed_unit=kn&models={model}",
        lats.join(","),
        lons.join(",")
    );
    let key = format!(
        "{model}_{step}_{:.2}_{:.2}_{:.2}_{:.2}",
        bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat
    );
    (url, key)
}

/// Choose the lattice step and snapped bbox for a viewport at `density` arrows
/// across, never finer than the model's `min_step`, coarsening until the lattice
/// fits under `MAX_POINTS`.
fn plan_lattice(bbox: Bbox, density: f32, min_step: f32) -> (Bbox, f32) {
    let clamped = bbox.clamp_valid();
    let mut step = nice_step(clamped.width().max(clamped.height()), density, min_step);
    let mut snapped = clamped.snap(step);
    while point_count(snapped, step) > MAX_POINTS {
        step *= 2.0;
        snapped = clamped.snap(step);
    }
    (snapped, step)
}

/// Holds the current wind field and refetches when the snapped viewport changes.
/// Mirrors `TileCache`: an async loader task feeds a held value. Refetches are
/// debounced while the view moves and backed off after a failure.
pub struct WindCache {
    cache_location: String,
    field: Option<WindField>,
    loaded_bbox: Option<Bbox>,
    loader: Option<(Bbox, Task<Option<WindField>>)>,
    /// The region we want but haven't fetched yet, and since when it's been the
    /// wanted region (for the settle debounce).
    pending: Option<(Bbox, f32, Instant)>,
    /// Don't fetch again until this time, set after a failed fetch.
    retry_after: Option<Instant>,
    /// Current model + density; changing either invalidates and refetches.
    model: WindModel,
    density: f32,
}

impl WindCache {
    pub fn new(cache_location: String) -> Self {
        Self {
            cache_location,
            field: None,
            loaded_bbox: None,
            loader: None,
            pending: None,
            retry_after: None,
            model: WindModel::EcmwfIfs,
            density: 10.0,
        }
    }

    /// Ask for wind over `bbox` for `model` at `density` arrows across. Changing
    /// model or density invalidates and refetches. Plans a lattice; once the
    /// wanted region has settled (and we're not loading or backing off), kick an
    /// async fetch. Finalizes a completed fetch first, keeping the old field on
    /// failure.
    pub fn request(&mut self, bbox: Bbox, model: WindModel, density: f32) {
        let now = Instant::now();

        if model != self.model || density != self.density {
            self.model = model;
            self.density = density;
            self.loader = None;
            self.loaded_bbox = None;
            self.pending = None;
            self.retry_after = None;
        }

        if let Some((b, task)) = &mut self.loader {
            if let Some(result) = task.try_take() {
                let b = *b;
                self.loader = None;
                match result {
                    Some(field) => {
                        self.field = Some(field);
                        self.loaded_bbox = Some(b);
                        self.retry_after = None;
                    }
                    // Keep the last good field on screen and retry after a delay,
                    // rather than freezing on the failed region or spamming it.
                    None => self.retry_after = Some(now + BACKOFF),
                }
            }
        }

        let (snapped, step) = plan_lattice(bbox, self.density, self.model.native_step_deg());

        if self.loaded_bbox == Some(snapped) {
            self.pending = None;
            return;
        }
        // One fetch in flight at a time; still backing off; not settled yet.
        if self.loader.is_some() || self.retry_after.is_some_and(|t| now < t) {
            return;
        }
        match self.pending {
            Some((b, _, since)) if b == snapped => {
                if now.duration_since(since) < DEBOUNCE {
                    return;
                }
            }
            _ => {
                self.pending = Some((snapped, step, now));
                return;
            }
        }

        let (url, key) = open_meteo_url(self.model.id(), snapped, step);
        let cache_location = self.cache_location.clone();
        let task = spawn_task(async move {
            fetch_wind_json(Path::new(&cache_location), &key, &url)
                .await
                .and_then(|bytes| WindField::from_open_meteo_json(&bytes))
        });
        self.loader = Some((snapped, task));
        self.pending = None;
    }

    /// The most recently loaded field, if any.
    pub fn field(&self) -> Option<&WindField> {
        self.field.as_ref()
    }

    /// The snapped bbox the current field was fetched for, if any.
    pub fn loaded_bbox(&self) -> Option<Bbox> {
        self.loaded_bbox
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_expands_to_step_grid() {
        let b = Bbox { min_lon: 8.1, min_lat: 47.2, max_lon: 8.9, max_lat: 47.7 };
        let s = b.snap(0.5);
        assert!((s.min_lon - 8.0).abs() < 1e-4);
        assert!((s.min_lat - 47.0).abs() < 1e-4);
        assert!((s.max_lon - 9.0).abs() < 1e-4);
        assert!((s.max_lat - 48.0).abs() < 1e-4);
    }

    // Zooming out (wider span) must pick a coarser lattice; zooming in, a finer
    // one, floored at the smallest step.
    #[test]
    fn nice_step_tracks_zoom() {
        assert!(nice_step(360.0, 10.0, 0.1) > nice_step(10.0, 10.0, 0.1));
        assert!(nice_step(10.0, 10.0, 0.1) > nice_step(0.5, 10.0, 0.1));
        assert_eq!(nice_step(0.01, 10.0, 0.1), 0.1); // floored at min_step
    }

    // A high-res model (small min_step) can sample finer than the old 0.1 floor.
    #[test]
    fn finer_min_step_allows_denser() {
        // 2 deg span, 100 arrows across -> 0.02 ideal, allowed by a 0.01 floor.
        assert!(nice_step(2.0, 100.0, 0.01) < 0.1);
        // but the same request on a 0.25 model can't go below its native grid.
        assert_eq!(nice_step(2.0, 100.0, 0.25), 0.25);
    }

    // A viewport spilling past the world clamps to valid lat/lon so the fetch
    // never asks for out-of-range coordinates.
    #[test]
    fn clamp_keeps_coords_valid() {
        let b = Bbox { min_lon: -520.0, min_lat: -140.0, max_lon: 430.0, max_lat: 140.0 };
        let c = b.clamp_valid();
        assert!(c.min_lon >= -180.0 && c.max_lon <= 180.0);
        assert!(c.min_lat >= -85.0 && c.max_lat <= 85.0);
    }

    // Even a world-spanning viewport stays under the point cap (bounded url).
    #[test]
    fn lattice_stays_under_cap() {
        let world = Bbox { min_lon: -180.0, min_lat: -85.0, max_lon: 180.0, max_lat: 85.0 };
        let (snapped, step) = plan_lattice(world, 10.0, 0.25);
        assert!(point_count(snapped, step) <= MAX_POINTS);
    }

    // Same snapped bbox + step must produce a stable cache key (so disk cache hits).
    #[test]
    fn url_key_is_stable() {
        let b = Bbox { min_lon: 8.0, min_lat: 47.0, max_lon: 9.0, max_lat: 48.0 };
        let (_, k1) = open_meteo_url("ecmwf_ifs025", b, 0.5);
        let (_, k2) = open_meteo_url("ecmwf_ifs025", b, 0.5);
        assert_eq!(k1, k2);
        assert!(k1.contains("ecmwf_ifs025"));
    }

    // Points sit on the global lattice (multiples of step), pinned regardless of
    // pan: a step-1 grid over [0,2] must include the whole-degree lines.
    #[test]
    fn lattice_points_are_pinned() {
        let b = Bbox { min_lon: 0.0, min_lat: 0.0, max_lon: 2.0, max_lat: 2.0 };
        let (url, _) = open_meteo_url("ecmwf_ifs025", b, 1.0);
        assert!(url.contains("0.0000"));
        assert!(url.contains("1.0000"));
        assert!(url.contains("2.0000"));
    }

    // The url must request u/v-able fields in knots and the current step.
    #[test]
    fn url_requests_current_wind_in_knots() {
        let b = Bbox { min_lon: 8.0, min_lat: 47.0, max_lon: 9.0, max_lat: 48.0 };
        let (url, _) = open_meteo_url("ecmwf_ifs025", b, 0.5);
        assert!(url.contains("current=wind_speed_10m,wind_direction_10m"));
        assert!(url.contains("wind_speed_unit=kn"));
        assert!(url.contains("models=ecmwf_ifs025"));
    }
}
