use std::io::Read;
use std::path::Path;

use bzip2::read::MultiBzDecoder;
use sailor_platform::{http, platform};

use super::run_candidates;

const DWD_BASE: &str = "https://opendata.dwd.de/weather/nwp/icon-eu/grib";

/// DWD ICON-EU stores each parameter in its own bz2 file, so there is no byte
/// index — we GET the whole `U_10M`/`V_10M` file and decompress it. `var` is the
/// DWD field tag (`U_10M`/`V_10M`); `dir` its lowercase directory (`u_10m`).
fn field_url(date: &str, hh: u32, dir: &str, var: &str) -> String {
    format!(
        "{DWD_BASE}/{hh:02}/{dir}/\
         icon-eu_europe_regular-lat-lon_single-level_{date}{hh:02}_000_{var}.grib2.bz2"
    )
}

/// bzip2-decompress `bz2` bytes to raw GRIB2. `MultiBzDecoder` tolerates
/// concatenated streams. None on a malformed stream.
fn bunzip2(bz2: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    MultiBzDecoder::new(bz2).read_to_end(&mut out).ok()?;
    Some(out)
}

/// Fetch DWD ICON-EU 10u/10v for the latest published run, cache-as-you-go.
/// Tries run candidates newest-first: GET each field's bz2, decompress, cache
/// the decompressed GRIB2. Returns the two raw GRIB2 messages, or None.
pub async fn fetch_wind(cache_location: &str, now_unix: i64) -> Option<(Vec<u8>, Vec<u8>)> {
    for (date, hh) in run_candidates(now_unix, 1, 8) {
        let cache_u =
            Path::new(cache_location).join(format!("wind/icon_eu_{date}_{hh:02}_10u.grib2"));
        let cache_v =
            Path::new(cache_location).join(format!("wind/icon_eu_{date}_{hh:02}_10v.grib2"));
        let (cu, cv) = (cache_u.to_string_lossy(), cache_v.to_string_lossy());

        if let (Some(u), Some(v)) = (platform::read_bytes(&cu), platform::read_bytes(&cv)) {
            return Some((u, v));
        }

        let Some(u_bz2) = http::get(&field_url(&date, hh, "u_10m", "U_10M")).await else {
            continue; // run not published yet; try the previous one
        };
        let v_bz2 = http::get(&field_url(&date, hh, "v_10m", "V_10M")).await?;
        let u = bunzip2(&u_bz2)?;
        let v = bunzip2(&v_bz2)?;
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
    fn url_matches_dwd_layout() {
        let u = field_url("20260809", 0, "u_10m", "U_10M");
        assert_eq!(
            u,
            "https://opendata.dwd.de/weather/nwp/icon-eu/grib/00/u_10m/\
             icon-eu_europe_regular-lat-lon_single-level_2026080900_000_U_10M.grib2.bz2"
        );
    }

    // Decode a real bzip2 stream: `printf 'hello icon-eu\n' | bzip2 -c`.
    #[test]
    fn bunzip2_decodes() {
        let bz2: &[u8] = &[
            0x42, 0x5a, 0x68, 0x39, 0x31, 0x41, 0x59, 0x26, 0x53, 0x59, 0xd6, 0x03, 0x7c, 0x06,
            0x00, 0x00, 0x03, 0x51, 0x80, 0x00, 0x10, 0x40, 0x02, 0x0a, 0x65, 0x82, 0x00, 0x20,
            0x00, 0x31, 0x00, 0xd3, 0x4d, 0x05, 0x30, 0x0d, 0xa8, 0x02, 0x59, 0x78, 0x8d, 0x62,
            0x38, 0x5d, 0xc9, 0x14, 0xe1, 0x42, 0x43, 0x58, 0x0d, 0xf0, 0x18,
        ];
        assert_eq!(bunzip2(bz2).unwrap(), b"hello icon-eu\n");
    }
}
