use std::path::Path;

use sailor_platform::{http, platform};
use serde::Deserialize;

use super::{GribRange, run_candidates};

/// Find the `10u` and `10v` byte ranges in an ECMWF open-data `.index`
/// (one JSON object per line). None if either is absent.
pub fn find_wind_ranges(index_text: &str) -> Option<(GribRange, GribRange)> {
    #[derive(Deserialize)]
    struct Entry {
        param: String,
        #[serde(rename = "_offset")]
        offset: u64,
        #[serde(rename = "_length")]
        length: u64,
    }

    let mut u = None;
    let mut v = None;
    for line in index_text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(e) = serde_json::from_str::<Entry>(line) else {
            continue;
        };
        let range = GribRange {
            offset: e.offset,
            length: e.length,
        };
        match e.param.as_str() {
            "10u" => u = Some(range),
            "10v" => v = Some(range),
            _ => {}
        }
    }
    Some((u?, v?))
}

const ECMWF_BASE: &str = "https://data.ecmwf.int/forecasts";

/// Fetch ECMWF open-data 10u/10v for the latest published run, cache-as-you-go.
/// Tries run candidates newest-first: GET the index, locate 10u/10v, range-GET
/// each message. Returns the two raw GRIB2 messages, or None if no run resolves.
pub async fn fetch_wind(cache_location: &str, now_unix: i64) -> Option<(Vec<u8>, Vec<u8>)> {
    for (date, hh) in run_candidates(now_unix, 1, 8) {
        let stem =
            format!("{ECMWF_BASE}/{date}/{hh:02}z/ifs/0p25/oper/{date}{hh:02}0000-0h-oper-fc");
        let cache_u =
            Path::new(cache_location).join(format!("wind/ecmwf_{date}_{hh:02}_10u.grib2"));
        let cache_v =
            Path::new(cache_location).join(format!("wind/ecmwf_{date}_{hh:02}_10v.grib2"));
        let (cu, cv) = (cache_u.to_string_lossy(), cache_v.to_string_lossy());

        // serve a cached run without touching the network.
        if let (Some(u), Some(v)) = (platform::read_bytes(&cu), platform::read_bytes(&cv)) {
            return Some((u, v));
        }

        let Some(index) = http::get(&format!("{stem}.index")).await else {
            continue; // run not published yet; try the previous one
        };
        let Some((ur, vr)) = find_wind_ranges(&String::from_utf8_lossy(&index)) else {
            continue;
        };
        let grib = format!("{stem}.grib2");
        let u = http::get_range(&grib, ur.offset, ur.length).await?;
        let v = http::get_range(&grib, vr.offset, vr.length).await?;
        platform::write_bytes(&cu, &u);
        platform::write_bytes(&cv, &v);
        return Some((u, v));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_wind_ranges() {
        let idx = "\
{\"param\": \"10u\", \"_offset\": 20118299, \"_length\": 868687}
{\"param\": \"10v\", \"_offset\": 24199492, \"_length\": 864428}";
        let (u, v) = find_wind_ranges(idx).unwrap();
        assert_eq!(
            u,
            GribRange {
                offset: 20118299,
                length: 868687
            }
        );
        assert_eq!(
            v,
            GribRange {
                offset: 24199492,
                length: 864428
            }
        );
    }

    #[test]
    fn missing_param_is_none() {
        let idx = "{\"param\": \"10u\", \"_offset\": 1, \"_length\": 2}";
        assert!(find_wind_ranges(idx).is_none()); // no 10v
    }
}
