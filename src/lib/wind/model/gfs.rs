use std::path::Path;

use sailor_platform::{http, platform};

use super::{GribRange, run_candidates};

/// Find the 10 m `UGRD`/`VGRD` byte ranges in a NOMADS GFS `.idx`. Unlike
/// ECMWF's JSON index, a `.idx` line is colon-delimited and carries only a
/// start offset (`rec:offset:d=..:VAR:level:..`), so a record's length is the
/// next record's offset. 10 m winds sit mid-file, so a following record always
/// exists. None if either field is absent.
pub fn find_wind_ranges(idx_text: &str) -> Option<(GribRange, GribRange)> {
    // (start offset, var, level) per record, in file order.
    let recs: Vec<(u64, &str, &str)> = idx_text
        .lines()
        .filter_map(|line| {
            let mut f = line.splitn(6, ':');
            let _rec = f.next()?;
            let offset: u64 = f.next()?.parse().ok()?;
            let _date = f.next()?;
            let var = f.next()?;
            let level = f.next()?;
            Some((offset, var, level))
        })
        .collect();

    let range = |var: &str| {
        let i = recs
            .iter()
            .position(|(_, v, l)| *v == var && *l == "10 m above ground")?;
        let offset = recs[i].0;
        // length runs to the next record's start offset.
        let end = recs.get(i + 1)?.0;
        Some(GribRange {
            offset,
            length: end - offset,
        })
    };
    Some((range("UGRD")?, range("VGRD")?))
}

const GFS_BASE: &str = "https://nomads.ncep.noaa.gov/pub/data/nccf/com/gfs/prod";

/// Fetch NOMADS GFS 0.25° 10u/10v for the latest published run, cache-as-you-go.
/// Tries run candidates newest-first (GFS analysis lags a few hours, absorbed by
/// the walk): GET the `.idx`, locate the 10 m winds, range-GET each. Returns the
/// two raw GRIB2 messages, or None if no run resolves.
pub async fn fetch_wind(cache_location: &str, now_unix: i64) -> Option<(Vec<u8>, Vec<u8>)> {
    for (date, hh) in run_candidates(now_unix, 1, 8) {
        let stem = format!("{GFS_BASE}/gfs.{date}/{hh:02}/atmos/gfs.t{hh:02}z.pgrb2.0p25.f000");
        let cache_u = Path::new(cache_location).join(format!("wind/gfs_{date}_{hh:02}_10u.grib2"));
        let cache_v = Path::new(cache_location).join(format!("wind/gfs_{date}_{hh:02}_10v.grib2"));
        let (cu, cv) = (cache_u.to_string_lossy(), cache_v.to_string_lossy());

        if let (Some(u), Some(v)) = (platform::read_bytes(&cu), platform::read_bytes(&cv)) {
            return Some((u, v));
        }

        let Some(idx) = http::get(&format!("{stem}.idx")).await else {
            continue; // run not published yet; try the previous one
        };
        let Some((ur, vr)) = find_wind_ranges(&String::from_utf8_lossy(&idx)) else {
            continue;
        };
        let u = http::get_range(stem.as_str(), ur.offset, ur.length).await?;
        let v = http::get_range(stem.as_str(), vr.offset, vr.length).await?;
        platform::write_bytes(&cu, &u);
        platform::write_bytes(&cv, &v);
        return Some((u, v));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // A trimmed .idx: 10 m winds mid-file, so both have a following record.
    const IDX: &str = "\
592:327000000:d=2024010100:PRMSL:mean sea level:anl:
593:328756289:d=2024010100:UGRD:10 m above ground:anl:
594:329600000:d=2024010100:VGRD:10 m above ground:anl:
595:330400000:d=2024010100:GUST:surface:anl:";

    #[test]
    fn finds_10m_wind_ranges() {
        let (u, v) = find_wind_ranges(IDX).unwrap();
        // length = next record offset - this offset.
        assert_eq!(
            u,
            GribRange {
                offset: 328756289,
                length: 329600000 - 328756289
            }
        );
        assert_eq!(
            v,
            GribRange {
                offset: 329600000,
                length: 330400000 - 329600000
            }
        );
    }

    #[test]
    fn missing_field_is_none() {
        let idx = "593:1:d=2024010100:UGRD:10 m above ground:anl:\n594:2:d=..:GUST:surface:anl:";
        assert!(find_wind_ranges(idx).is_none()); // no 10 m VGRD
    }

    // A 10 m wind must not match another level's UGRD (e.g. 100 m).
    #[test]
    fn ignores_other_levels() {
        let idx = "1:0:d=2024010100:UGRD:100 m above ground:anl:\n2:500:d=..:VGRD:100 m above ground:anl:";
        assert!(find_wind_ranges(idx).is_none());
    }
}
