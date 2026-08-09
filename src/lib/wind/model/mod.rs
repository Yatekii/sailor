use crate::wind::WindModel;
use crate::wind::grid::WindGrid;

pub mod ecmwf;
pub mod gfs;
pub mod icon_ch1;
pub mod icon_eu;

/// A field's byte range within a source `.grib2` file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GribRange {
    pub offset: u64,
    pub length: u64,
}

/// (year, month, day, hour) UTC from a unix timestamp. Integer civil calendar
/// (Howard Hinnant's algorithm); no leap seconds.
pub fn utc_from_unix(secs: i64) -> (i32, u32, u32, u32) {
    let days = secs.div_euclid(86400);
    let hour = (secs.rem_euclid(86400) / 3600) as u32;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400) + if m <= 2 { 1 } else { 0 };
    (y as i32, m, d, hour)
}

/// Six-hourly run candidates `(yyyymmdd, hh)` newest-first for `now_unix`,
/// walking back `count` runs. Starts `skip` runs before now, since publication
/// lags the run time (ECMWF ~1 run, GFS more, absorbed by the walk).
pub fn run_candidates(now_unix: i64, skip: i64, count: i64) -> Vec<(String, u32)> {
    let run = now_unix.div_euclid(6 * 3600) * 6 * 3600;
    (skip..skip + count)
        .map(|i| {
            let (y, m, d, h) = utc_from_unix(run - i * 6 * 3600);
            (format!("{y:04}{m:02}{d:02}"), h)
        })
        .collect()
}

/// Load the latest wind field for `model` as a regular `WindGrid`, from its
/// native GRIB source, caching as it goes. None if no run resolves or the model
/// has no GRIB source. Regular lat/lon sources decode via `from_uv_messages`;
/// unstructured sources (ICON-CH1) build the grid by regridding their mesh.
pub async fn load_grid(
    model: WindModel,
    cache_location: &str,
    now_unix: i64,
) -> Option<WindGrid> {
    match model {
        WindModel::EcmwfIfs => uv(ecmwf::fetch_wind(cache_location, now_unix).await),
        WindModel::Gfs => uv(gfs::fetch_wind(cache_location, now_unix).await),
        WindModel::IconEu => uv(icon_eu::fetch_wind(cache_location, now_unix).await),
        WindModel::IconCh1 => icon_ch1::load_grid(cache_location).await,
        // Open-Meteo models have no native GRIB source here yet.
        _ => None,
    }
}

/// Decode a fetched (10u, 10v) message pair into a regular grid.
fn uv(msgs: Option<(Vec<u8>, Vec<u8>)>) -> Option<WindGrid> {
    let (u, v) = msgs?;
    WindGrid::from_uv_messages(&u, &v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_epoch_and_rollover() {
        assert_eq!(utc_from_unix(0), (1970, 1, 1, 0));
        assert_eq!(utc_from_unix(86400 + 3600), (1970, 1, 2, 1));
        assert_eq!(utc_from_unix(951_782_400), (2000, 2, 29, 0)); // leap day
    }

    #[test]
    fn run_candidates_are_six_hourly_newest_first() {
        let c = run_candidates(1_754_611_200, 1, 8); // arbitrary fixed time
        assert_eq!(c.len(), 8);
        for (_, hh) in &c {
            assert_eq!(hh % 6, 0);
        }
        // strictly older as we go: parse yyyymmddHH into a comparable number.
        let key = |(d, h): &(String, u32)| format!("{d}{h:02}").parse::<u64>().unwrap();
        for w in c.windows(2) {
            assert!(key(&w[0]) > key(&w[1]));
        }
    }
}
