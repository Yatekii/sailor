use serde::Deserialize;

/// A field's byte range within an ECMWF open-data `.grib2` file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GribRange {
    pub offset: u64,
    pub length: u64,
}

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
        let range = GribRange { offset: e.offset, length: e.length };
        match e.param.as_str() {
            "10u" => u = Some(range),
            "10v" => v = Some(range),
            _ => {}
        }
    }
    Some((u?, v?))
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

/// ECMWF run candidates `(yyyymmdd, hh)` newest-first for `now_unix`, walking
/// back 8 six-hourly runs. Starts one run before now, since publication lags.
pub fn ecmwf_run_candidates(now_unix: i64) -> Vec<(String, u32)> {
    let run = now_unix.div_euclid(6 * 3600) * 6 * 3600;
    (1..=8)
        .map(|i| {
            let (y, m, d, h) = utc_from_unix(run - i * 6 * 3600);
            (format!("{y:04}{m:02}{d:02}"), h)
        })
        .collect()
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
        assert_eq!(u, GribRange { offset: 20118299, length: 868687 });
        assert_eq!(v, GribRange { offset: 24199492, length: 864428 });
    }

    #[test]
    fn missing_param_is_none() {
        let idx = "{\"param\": \"10u\", \"_offset\": 1, \"_length\": 2}";
        assert!(find_wind_ranges(idx).is_none()); // no 10v
    }

    #[test]
    fn utc_epoch_and_rollover() {
        assert_eq!(utc_from_unix(0), (1970, 1, 1, 0));
        assert_eq!(utc_from_unix(86400 + 3600), (1970, 1, 2, 1));
        assert_eq!(utc_from_unix(951_782_400), (2000, 2, 29, 0)); // leap day
    }

    #[test]
    fn run_candidates_are_six_hourly_newest_first() {
        let c = ecmwf_run_candidates(1_754_611_200); // arbitrary fixed time
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
