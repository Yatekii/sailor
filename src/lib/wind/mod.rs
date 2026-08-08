use serde::Deserialize;

pub mod cache;
pub mod ecmwf;
pub mod grid;

/// A selectable weather model, mapped to its Open-Meteo id. Non-US models first;
/// GFS is the US one we prefer to avoid but keep available.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindModel {
    EcmwfIfs,
    Icon,
    AromeFrance,
    IconCh1,
    Gfs,
}

impl WindModel {
    pub const ALL: [WindModel; 5] = [
        Self::EcmwfIfs,
        Self::Icon,
        Self::AromeFrance,
        Self::IconCh1,
        Self::Gfs,
    ];

    /// The Open-Meteo `models=` id.
    pub fn id(self) -> &'static str {
        match self {
            Self::EcmwfIfs => "ecmwf_ifs025",
            Self::Icon => "icon_seamless",
            Self::AromeFrance => "meteofrance_arome_france",
            // meteoswiss ids are newer; verify against open-meteo's model list if
            // this one returns nothing.
            Self::IconCh1 => "meteoswiss_icon_ch1",
            Self::Gfs => "gfs_seamless",
        }
    }

    /// Display name including the native grid resolution (kept in sync with
    /// `native_step_deg`; ~111 km per degree).
    pub fn label(self) -> &'static str {
        match self {
            Self::EcmwfIfs => "ECMWF IFS · global · 0.25° (~28 km)",
            Self::Icon => "ICON · global (DWD) · 0.1° (~11 km)",
            Self::AromeFrance => "AROME · France · 0.025° (~2.8 km)",
            Self::IconCh1 => "ICON-CH1 · Alps (MeteoSwiss) · 0.01° (~1 km)",
            Self::Gfs => "GFS · global (US) · 0.25° (~28 km)",
        }
    }

    /// The model's native grid spacing in degrees. Sampling finer than this just
    /// returns interpolated duplicates, so it's the floor for the arrow lattice.
    pub fn native_step_deg(self) -> f32 {
        match self {
            Self::EcmwfIfs | Self::Gfs => 0.25,
            Self::Icon => 0.1,
            Self::AromeFrance => 0.025,
            Self::IconCh1 => 0.01,
        }
    }
}

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

impl WindField {
    /// Parse Open-Meteo's multi-location `current` response into a field.
    /// Returns None if the body is not the expected JSON shape.
    pub fn from_open_meteo_json(bytes: &[u8]) -> Option<WindField> {
        #[derive(Deserialize)]
        struct Current {
            wind_speed_10m: f32,
            wind_direction_10m: f32,
        }

        #[derive(Deserialize)]
        struct Loc {
            latitude: f32,
            longitude: f32,
            current: Current,
        }

        let locs: Vec<Loc> = serde_json::from_slice(bytes).ok()?;
        let samples = locs
            .into_iter()
            .map(|l| {
                let (u, v) = uv_from_speed_dir(l.current.wind_speed_10m, l.current.wind_direction_10m);
                WindSample { lon: l.longitude, lat: l.latitude, u, v }
            })
            .collect();
        Some(WindField { samples })
    }
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

    // Open-Meteo returns a JSON array (one object per requested location), each
    // with a `current` block. We map each into one WindSample.
    #[test]
    fn parses_open_meteo_array() {
        let json = br#"[
          {"latitude":47.0,"longitude":8.0,
           "current":{"wind_speed_10m":10.0,"wind_direction_10m":270.0}},
          {"latitude":47.5,"longitude":8.5,
           "current":{"wind_speed_10m":0.0,"wind_direction_10m":0.0}}
        ]"#;
        let field = WindField::from_open_meteo_json(json).unwrap();
        assert_eq!(field.samples.len(), 2);
        let s = field.samples[0];
        assert!((s.lon - 8.0).abs() < 1e-3);
        assert!((s.lat - 47.0).abs() < 1e-3);
        assert!((s.u - 10.0).abs() < 1e-3); // westerly -> eastward
        assert!(s.v.abs() < 1e-3);
    }
}
