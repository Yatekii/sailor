use std::path::Path;

use crate::platform::{Task, spawn_task};
use crate::wind::WindField;
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
    /// Expand outward to the nearest `step`-degree grid lines, so the fetched
    /// region only changes when you pan across a grid line (bounds refetch spam,
    /// makes the disk cache key stable).
    pub fn snap(&self, step: f32) -> Bbox {
        Bbox {
            min_lon: (self.min_lon / step).floor() * step,
            min_lat: (self.min_lat / step).floor() * step,
            max_lon: (self.max_lon / step).ceil() * step,
            max_lat: (self.max_lat / step).ceil() * step,
        }
    }
}

/// Build the Open-Meteo request URL for a `cols`x`rows` grid over `bbox`, plus a
/// stable disk-cache key. Requests the current step, wind in knots.
pub fn open_meteo_url(model: &str, bbox: Bbox, cols: u32, rows: u32) -> (String, String) {
    let mut lats = Vec::new();
    let mut lons = Vec::new();
    for r in 0..rows {
        let ty = r as f32 / (rows.max(2) - 1) as f32;
        let lat = bbox.min_lat + ty * (bbox.max_lat - bbox.min_lat);
        for c in 0..cols {
            let tx = c as f32 / (cols.max(2) - 1) as f32;
            let lon = bbox.min_lon + tx * (bbox.max_lon - bbox.min_lon);
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
        "{model}_{:.2}_{:.2}_{:.2}_{:.2}_{cols}x{rows}",
        bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat
    );
    (url, key)
}

/// Grid resolution and snap step for the fixed-model overlay.
const GRID_COLS: u32 = 12;
const GRID_ROWS: u32 = 8;
const SNAP_STEP: f32 = 0.25;
const MODEL: &str = "ecmwf_ifs025";

/// Holds the current wind field and refetches when the snapped viewport changes.
/// Mirrors `TileCache`: an async loader task feeds a held value.
pub struct WindCache {
    cache_location: String,
    field: Option<WindField>,
    loaded_bbox: Option<Bbox>,
    loader: Option<(Bbox, Task<Option<WindField>>)>,
}

impl WindCache {
    pub fn new(cache_location: String) -> Self {
        Self {
            cache_location,
            field: None,
            loaded_bbox: None,
            loader: None,
        }
    }

    /// Ask for wind over `bbox`. Snaps it; if that differs from what we have (or
    /// are loading), kick an async fetch. Finalizes any completed fetch first.
    pub fn request(&mut self, bbox: Bbox) {
        if let Some((b, task)) = &mut self.loader {
            if let Some(result) = task.try_take() {
                let b = *b;
                self.loader = None;
                if let Some(field) = result {
                    self.field = Some(field);
                }
                // mark the bbox even on failure so we don't immediately re-fetch
                // the same region and spam the api at ~60 req/sec
                self.loaded_bbox = Some(b);
            }
        }

        let snapped = bbox.snap(SNAP_STEP);
        let already = self.loaded_bbox == Some(snapped);
        let loading = self.loader.as_ref().map(|(b, _)| *b) == Some(snapped);
        if already || loading {
            return;
        }

        let (url, key) = open_meteo_url(MODEL, snapped, GRID_COLS, GRID_ROWS);
        let cache_location = self.cache_location.clone();
        let task = spawn_task(async move {
            fetch_wind_json(Path::new(&cache_location), &key, &url)
                .await
                .and_then(|bytes| WindField::from_open_meteo_json(&bytes))
        });
        self.loader = Some((snapped, task));
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

    // Same snapped bbox + model must produce a stable cache key (so disk cache hits).
    #[test]
    fn url_key_is_stable() {
        let b = Bbox { min_lon: 8.0, min_lat: 47.0, max_lon: 9.0, max_lat: 48.0 };
        let (_, k1) = open_meteo_url("ecmwf_ifs025", b, 4, 4);
        let (_, k2) = open_meteo_url("ecmwf_ifs025", b, 4, 4);
        assert_eq!(k1, k2);
        assert!(k1.contains("ecmwf_ifs025"));
    }

    // The url must request u/v-able fields in knots and the current step.
    #[test]
    fn url_requests_current_wind_in_knots() {
        let b = Bbox { min_lon: 8.0, min_lat: 47.0, max_lon: 9.0, max_lat: 48.0 };
        let (url, _) = open_meteo_url("ecmwf_ifs025", b, 2, 2);
        assert!(url.contains("current=wind_speed_10m,wind_direction_10m"));
        assert!(url.contains("wind_speed_unit=kn"));
        assert!(url.contains("models=ecmwf_ifs025"));
        // 2x2 grid -> 4 comma-joined latitudes.
        assert!(url.matches("47").count() + url.matches("48").count() >= 4);
    }
}
