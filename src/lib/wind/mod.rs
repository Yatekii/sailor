/// One wind sample on the lat/lon grid. u is eastward, v is northward, in knots.
#[derive(Clone, Copy, Debug)]
pub struct WindSample {
    pub lon: f32,
    pub lat: f32,
    pub u: f32,
    pub v: f32,
}

/// A gridded wind field at one forecast time. Source-agnostic: nothing here
/// knows whether the numbers came from Open-Meteo, GRIB, or a test.
#[derive(Clone, Debug, Default)]
pub struct WindField {
    pub samples: Vec<WindSample>,
}

/// Convert Open-Meteo speed + meteorological direction (the direction wind comes
/// FROM) into u (eastward) and v (northward) components. A flipped sign here
/// points every arrow backwards, hence the tests.
pub fn uv_from_speed_dir(speed: f32, dir_deg: f32) -> (f32, f32) {
    let r = dir_deg.to_radians();
    (-speed * r.sin(), -speed * r.cos())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-3, "{a} != {b}");
    }

    // Meteorological convention: direction is where wind comes FROM.
    // A northerly (from north, dir=0) blows toward the south: v negative, u ~0.
    #[test]
    fn northerly_points_south() {
        let (u, v) = uv_from_speed_dir(10.0, 0.0);
        approx(u, 0.0);
        approx(v, -10.0);
    }

    // A westerly (from west, dir=270) blows toward the east: u positive, v ~0.
    #[test]
    fn westerly_points_east() {
        let (u, v) = uv_from_speed_dir(10.0, 270.0);
        approx(u, 10.0);
        approx(v, 0.0);
    }
}
