use gribberish::message::read_messages;
use std::f32::consts::PI;

/// A decoded regular lat/lon wind field for one forecast step. Degrees for
/// angles, knots for u/v. The lat axis descends from `lat0` by `lat_step`; the
/// lon axis ascends from `lon0` by `lon_step`. May be global (spans 360°, wraps
/// at the seam) or regional (spans less; `sample` clamps outside coverage). Data
/// is row-major (lat-major, lon fastest).
#[derive(Clone, Debug)]
pub struct WindGrid {
    pub nlat: usize,
    pub nlon: usize,
    pub lat0: f32,
    pub lat_step: f32,
    pub lon0: f32,
    pub lon_step: f32,
    pub u: Vec<f32>,
    pub v: Vec<f32>,
}

impl WindGrid {
    /// Nearest-neighbour sample at `lon`/`lat` in degrees. Row clamps to the
    /// poles. Longitude is periodic (mod 360°): a global grid wraps around the
    /// seam, a regional grid (span < 360°) clamps to its covered range so points
    /// land in the right column instead of aliasing modulo the column count.
    pub fn sample(&self, lon: f32, lat: f32) -> (f32, f32) {
        let row = (((lat - self.lat0) / self.lat_step).round() as isize)
            .clamp(0, self.nlat as isize - 1) as usize;
        // bring (lon - lon0) into [0, 360) before indexing.
        let deg = (lon - self.lon0).rem_euclid(360.0);
        let raw = (deg / self.lon_step).round() as isize;
        let span = self.nlon as f32 * self.lon_step; // ~360 for a global grid
        let col = if span.abs() >= 359.9 {
            raw.rem_euclid(self.nlon as isize)
        } else {
            raw.clamp(0, self.nlon as isize - 1)
        } as usize;
        let idx = row * self.nlon + col;
        (self.u[idx], self.v[idx])
    }
}

/// Metres/second to knots.
pub(crate) const MS_TO_KN: f32 = 1.943_844_5;

impl WindGrid {
    /// Build a grid from 10u and 10v GRIB2 messages (ECMWF, GFS, any regular
    /// lat/lon source). Returns None if either message is missing, not a regular
    /// lat/lon grid, or the field lengths disagree. Values are m/s -> knots.
    pub fn from_uv_messages(u_bytes: &[u8], v_bytes: &[u8]) -> Option<WindGrid> {
        let um = read_messages(u_bytes).next()?;
        let vm = read_messages(v_bytes).next()?;
        let (nlat, nlon) = um.grid_dimensions().ok()?;
        let proj = um.latlng_projector().ok()?;
        if !proj.is_regular_latlng_grid() {
            return None;
        }
        let (lats, lons) = proj.lat_lng();
        if lats.len() < 2 || lons.len() < 2 {
            return None;
        }
        let ud = um.data().ok()?;
        let vd = vm.data().ok()?;
        if ud.len() != nlat * nlon || vd.len() != nlat * nlon {
            return None;
        }
        Some(WindGrid {
            nlat,
            nlon,
            lat0: lats[0] as f32,
            lat_step: (lats[1] - lats[0]) as f32,
            lon0: lons[0] as f32,
            lon_step: (lons[1] - lons[0]) as f32,
            u: ud.iter().map(|x| *x as f32 * MS_TO_KN).collect(),
            v: vd.iter().map(|x| *x as f32 * MS_TO_KN).collect(),
        })
    }
}

impl WindGrid {
    /// Build a regular grid from scattered cells by averaging every cell that
    /// falls in each target texel. `bbox` is (min_lon, min_lat, max_lon,
    /// max_lat); `step` the texel size in degrees. Row 0 is north. Cells
    /// outside the box are ignored; texels with no cell stay zero. u/v are
    /// taken as-is (caller converts units).
    pub fn from_scattered(
        lats: &[f32],
        lons: &[f32],
        u: &[f32],
        v: &[f32],
        bbox: (f32, f32, f32, f32),
        step: f32,
    ) -> WindGrid {
        let (min_lon, min_lat, max_lon, max_lat) = bbox;
        let nlon = (((max_lon - min_lon) / step).round() as usize).max(1) + 1;
        let nlat = (((max_lat - min_lat) / step).round() as usize).max(1) + 1;
        let mut su = vec![0.0f32; nlat * nlon];
        let mut sv = vec![0.0f32; nlat * nlon];
        let mut cnt = vec![0u32; nlat * nlon];
        for i in 0..lats.len() {
            let (lat, lon) = (lats[i], lons[i]);
            if lon < min_lon || lon > max_lon || lat < min_lat || lat > max_lat {
                continue;
            }
            let col = ((lon - min_lon) / step).round() as usize;
            // row 0 = north, so measure down from max_lat.
            let row = ((max_lat - lat) / step).round() as usize;
            if row >= nlat || col >= nlon {
                continue;
            }
            let idx = row * nlon + col;
            su[idx] += u[i];
            sv[idx] += v[i];
            cnt[idx] += 1;
        }
        for idx in 0..su.len() {
            if cnt[idx] > 0 {
                let n = cnt[idx] as f32;
                su[idx] /= n;
                sv[idx] /= n;
            }
        }
        WindGrid {
            nlat,
            nlon,
            lat0: max_lat,
            lat_step: -step,
            lon0: min_lon,
            lon_step: step,
            u: su,
            v: sv,
        }
    }
}

impl WindGrid {
    /// Resample into a `w`x`h` mercator-space u/v field (interleaved u,v),
    /// row 0 = north. World y in [0,1] maps to latitude by the inverse mercator
    /// (clamped to ±85°), longitude spans the globe. Sampling the result with
    /// `(fract(world.x), world.y)` then needs no projection math on the gpu.
    pub fn resample_mercator(&self, w: usize, h: usize) -> Vec<f32> {
        let mut out = vec![0.0f32; w * h * 2];
        for j in 0..h {
            let world_y = (j as f32 + 0.5) / h as f32;
            // inverse mercator: lat from normalized y (0=north, 1=south).
            let lat_rad = 2.0 * ((PI * (1.0 - 2.0 * world_y)).exp().atan() - PI / 4.0);
            let lat = lat_rad.to_degrees().clamp(-85.0, 85.0);
            for i in 0..w {
                let lon = (i as f32 + 0.5) / w as f32 * 360.0 - 180.0;
                let (u, v) = self.sample(lon, lat);
                let idx = (j * w + i) * 2;
                out[idx] = u;
                out[idx + 1] = v;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2 rows (lat 90, 89) x 4 cols (lon 0,90,180,270), u = index, v = -index.
    fn grid() -> WindGrid {
        let u: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let v: Vec<f32> = (0..8).map(|i| -(i as f32)).collect();
        WindGrid {
            nlat: 2,
            nlon: 4,
            lat0: 90.0,
            lat_step: -1.0,
            lon0: 0.0,
            lon_step: 90.0,
            u,
            v,
        }
    }

    #[test]
    fn samples_nearest_cell() {
        let g = grid();
        assert_eq!(g.sample(0.0, 90.0), (0.0, 0.0)); // row0 col0
        assert_eq!(g.sample(90.0, 90.0), (1.0, -1.0)); // row0 col1
        assert_eq!(g.sample(0.0, 89.0), (4.0, -4.0)); // row1 col0
    }

    #[test]
    fn longitude_wraps() {
        let g = grid();
        // -90 == 270 -> col3
        assert_eq!(g.sample(-90.0, 90.0), (3.0, -3.0));
        // 360 == 0 -> col0
        assert_eq!(g.sample(360.0, 90.0), (0.0, 0.0));
    }

    #[test]
    fn latitude_clamps() {
        let g = grid();
        // far south clamps to last row
        assert_eq!(g.sample(0.0, -200.0), (4.0, -4.0));
    }

    // A regional grid (span < 360°) whose lon origin is in 0..360 must index by
    // periodic longitude, not wrap modulo the column count. Mirrors ICON-EU:
    // lon0 = 336.5 (= -23.5°), 0.0625° step. A point at 8.5°E must land in the
    // column for 8.5°, not alias into the Atlantic.
    #[test]
    fn regional_grid_indexes_by_periodic_lon() {
        let nlon = 1377;
        let lon0 = 336.5;
        let step = 0.0625;
        // one row; u = column index so we can read back which column was hit.
        let u: Vec<f32> = (0..nlon).map(|c| c as f32).collect();
        let v = vec![0.0; nlon];
        let g = WindGrid {
            nlat: 1,
            nlon,
            lat0: 47.0,
            lat_step: -0.0625,
            lon0,
            lon_step: step,
            u,
            v,
        };
        // expected column for 8.5°E: ((8.5 - 336.5) mod 360) / step.
        let expected = (((8.5f32 - lon0).rem_euclid(360.0)) / step).round();
        let (col_hit, _) = g.sample(8.5, 47.0);
        assert_eq!(col_hit, expected);
        // and that column really is ~8.5°E, not a wrapped-around value.
        let hit_lon = lon0 + col_hit * step - 360.0;
        assert!(
            (hit_lon - 8.5).abs() < step,
            "sampled {hit_lon}°, wanted 8.5°"
        );
    }

    #[test]
    fn decodes_ecmwf_fixture() {
        let u = std::fs::read("tests/fixtures/wind/ecmwf_10u.grib2").unwrap();
        let v = std::fs::read("tests/fixtures/wind/ecmwf_10v.grib2").unwrap();
        let g = WindGrid::from_uv_messages(&u, &v).expect("decode");
        assert_eq!((g.nlat, g.nlon), (721, 1440));
        assert!((g.lat0 - 90.0).abs() < 1e-3);
        assert!((g.lat_step - -0.25).abs() < 1e-3);
        assert!((g.lon0 - 180.0).abs() < 1e-3);
        assert!((g.lon_step - 0.25).abs() < 1e-3);
        assert_eq!(g.u.len(), 721 * 1440);
        assert_eq!(g.v.len(), 721 * 1440);
        // wind should be a sane magnitude in knots everywhere.
        let (u0, v0) = g.sample(8.5, 47.4); // zurich
        assert!(u0.hypot(v0) < 200.0);
    }

    // Scattered cells averaged into regular texels. Two cells fall in the same
    // target texel; their u/v must average. A far-away cell must not leak in.
    #[test]
    fn from_scattered_averages_cells_in_texel() {
        // target: lon [0,1], lat [0,1], step 0.5 -> 3x3 texels, centers on the
        // 0.0/0.5/1.0 lines. Two cells near (0.5,0.5) and one near (0.0,0.0).
        let lats = [0.51, 0.49, 0.01];
        let lons = [0.51, 0.49, 0.01];
        let u = [10.0, 20.0, -5.0];
        let v = [1.0, 3.0, 0.0];
        let g = WindGrid::from_scattered(&lats, &lons, &u, &v, (0.0, 0.0, 1.0, 1.0), 0.5);
        assert_eq!((g.nlat, g.nlon), (3, 3));
        // center texel (lon 0.5, lat 0.5) averages the first two cells.
        let (cu, cv) = g.sample(0.5, 0.5);
        assert!((cu - 15.0).abs() < 1e-3, "u avg got {cu}");
        assert!((cv - 2.0).abs() < 1e-3, "v avg got {cv}");
        // corner texel (lon 0, lat 0) holds the third cell only.
        let (du, _) = g.sample(0.0, 0.0);
        assert!((du + 5.0).abs() < 1e-3, "corner u got {du}");
    }

    #[test]
    fn resample_matches_grid_at_texel() {
        // small synthetic global grid: u = lon, v = lat, so we can predict samples.
        let nlat = 181;
        let nlon = 360;
        let mut u = vec![0.0f32; nlat * nlon];
        let mut v = vec![0.0f32; nlat * nlon];
        for r in 0..nlat {
            for c in 0..nlon {
                u[r * nlon + c] = c as f32; // "lon index"
                v[r * nlon + c] = 90.0 - r as f32; // latitude
            }
        }
        let g = WindGrid {
            nlat,
            nlon,
            lat0: 90.0,
            lat_step: -1.0,
            lon0: 0.0,
            lon_step: 1.0,
            u,
            v,
        };
        let w = 64;
        let h = 64;
        let out = g.resample_mercator(w, h);
        assert_eq!(out.len(), w * h * 2);
        // row 32 center is world_y ≈ 0.508 → lat ≈ -2.8°, so |v| < 4.
        let j = 32;
        let i = 10;
        let vv = out[(j * w + i) * 2 + 1];
        assert!(vv.abs() < 4.0, "equator lat ~0, got {vv}");
    }
}
