/// A decoded regular lat/lon wind field for one forecast step. Degrees for
/// angles, knots for u/v. The lat axis descends from `lat0` by `lat_step`; the
/// lon axis ascends from `lon0` by `lon_step` and wraps the globe, so longitude
/// is indexed modulo `nlon`. Data is row-major (lat-major, lon fastest).
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
    /// Nearest-neighbour sample at `lon`/`lat` in degrees. Longitude wraps; row
    /// clamps to the poles.
    pub fn sample(&self, lon: f32, lat: f32) -> (f32, f32) {
        let row = (((lat - self.lat0) / self.lat_step).round() as isize)
            .clamp(0, self.nlat as isize - 1) as usize;
        let raw = ((lon - self.lon0) / self.lon_step).round() as isize;
        let col = raw.rem_euclid(self.nlon as isize) as usize;
        let idx = row * self.nlon + col;
        (self.u[idx], self.v[idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2 rows (lat 90, 89) x 4 cols (lon 0,90,180,270), u = index, v = -index.
    fn grid() -> WindGrid {
        let u: Vec<f32> = (0..8).map(|i| i as f32).collect();
        let v: Vec<f32> = (0..8).map(|i| -(i as f32)).collect();
        WindGrid { nlat: 2, nlon: 4, lat0: 90.0, lat_step: -1.0, lon0: 0.0, lon_step: 90.0, u, v }
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
}
