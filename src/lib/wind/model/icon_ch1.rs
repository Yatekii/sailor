use std::collections::HashMap;

use sailor_platform::http;
use serde::Deserialize;

use crate::wind::grid::{MS_TO_KN, WindGrid};
use gribberish::message::read_messages;
use sailor_platform::platform;

const STAC: &str = "https://data.geo.admin.ch/api/stac/v1";
const CID: &str = "ch.meteoschweiz.ogd-forecasting-icon-ch1";

/// Swiss regrid target: (min_lon, min_lat, max_lon, max_lat) and texel step in
/// degrees. 0.02° (~2.2 km) is coarser than the ~1 km mesh, so every texel
/// captures at least one cell (no holes) while staying far finer than ICON-EU.
/// ponytail: bump toward 0.01° with a hole-fill pass if 2 km reads too soft.
const BOX: (f32, f32, f32, f32) = (5.5, 45.5, 11.0, 48.0);
const STEP: f32 = 0.02;

/// GRIB2 (category, parameter) of the per-cell coordinate fields in the
/// constants file: latitude and longitude in degrees (discipline 0).
const LAT_ID: (u8, u8) = (191, 1);
const LON_ID: (u8, u8) = (191, 2);

#[derive(Deserialize)]
struct Asset {
    href: String,
}

#[derive(Deserialize)]
struct Props {
    #[serde(rename = "forecast:reference_datetime")]
    reference: String,
}

#[derive(Deserialize)]
struct Feature {
    properties: Props,
    assets: HashMap<String, Asset>,
}

#[derive(Deserialize)]
struct SearchResp {
    features: Vec<Feature>,
}

/// Build the exact STAC `title` fragment for a control, step-0 field of `var`
/// at a reference time. `reference_iso` is STAC's `YYYY-MM-DDTHH:MM:SSZ`.
/// Returns None if the timestamp is malformed. Example output:
/// `U_10M at 08.08.2026 15:00 Step 0 (Control)`.
fn title_fragment(reference_iso: &str, var: &str) -> Option<String> {
    // YYYY-MM-DDTHH:MM:SSZ
    let b = reference_iso.as_bytes();
    if reference_iso.len() < 16 || b[4] != b'-' || b[10] != b'T' {
        return None;
    }
    let (y, m, d) = (&reference_iso[0..4], &reference_iso[5..7], &reference_iso[8..10]);
    let (hh, mi) = (&reference_iso[11..13], &reference_iso[14..16]);
    Some(format!("{var} at {d}.{m}.{y} {hh}:{mi} Step 0 (Control)"))
}

/// The single asset href of a STAC feature (each ICON-CH1 item has exactly one).
fn first_href(f: &Feature) -> Option<String> {
    f.assets.values().next().map(|a| a.href.clone())
}

/// POST a STAC search body and parse the response.
async fn search(body: &str) -> Option<SearchResp> {
    let bytes = http::post_json(&format!("{STAC}/search"), body).await?;
    serde_json::from_slice(&bytes).ok()
}

/// Discover the latest run's control step-0 `U_10M`/`V_10M` signed hrefs.
async fn discover_wind_hrefs() -> Option<(String, String)> {
    // newest U_10M item -> its reference datetime.
    let newest = search(
        r#"{"collections":["ch.meteoschweiz.ogd-forecasting-icon-ch1"],"query":{"title":{"startsWith":"U_10M"}},"limit":1}"#,
    )
    .await?;
    let reference = newest.features.first()?.properties.reference.clone();

    let mut hrefs = [None, None];
    for (i, var) in ["U_10M", "V_10M"].iter().enumerate() {
        let frag = title_fragment(&reference, var)?;
        let body = format!(
            r#"{{"collections":["{CID}"],"query":{{"title":{{"contains":"{frag}"}}}},"limit":1}}"#
        );
        let resp = search(&body).await?;
        hrefs[i] = first_href(resp.features.first()?);
    }
    Some((hrefs[0].take()?, hrefs[1].take()?))
}

/// Discover the collection-level horizontal-constants (grid) signed href.
async fn discover_constants_href() -> Option<String> {
    #[derive(Deserialize)]
    struct Collection {
        assets: HashMap<String, Asset>,
    }
    let bytes = http::get(&format!("{STAC}/collections/{CID}")).await?;
    let c: Collection = serde_json::from_slice(&bytes).ok()?;
    c.assets
        .get("horizontal_constants_icon-ch1-eps.grib2")
        .map(|a| a.href.clone())
}

/// Find the message whose (category, parameter) is `id` and return its values.
fn coord_field(bytes: &[u8], id: (u8, u8)) -> Option<Vec<f32>> {
    for m in read_messages(bytes) {
        let cat = m.category_value().ok()?;
        let par = m.parameter_value().ok()?;
        if m.discipline_value().ok()? == 0 && (cat, par) == id {
            let d = m.data().ok()?;
            return Some(d.into_iter().map(|x| x as f32).collect());
        }
    }
    None
}

/// Per-cell (lat, lon) in degrees, cached as raw f32 so the 34 MB constants
/// file is fetched at most once ever. Returns (lats, lons) of equal length.
async fn cell_coords(cache_location: &str) -> Option<(Vec<f32>, Vec<f32>)> {
    let lat_path = format!("{cache_location}/wind/icon_ch1_lat.f32");
    let lon_path = format!("{cache_location}/wind/icon_ch1_lon.f32");
    if let (Some(la), Some(lo)) =
        (platform::read_bytes(&lat_path), platform::read_bytes(&lon_path))
    {
        return Some((f32_from_bytes(&la), f32_from_bytes(&lo)));
    }
    let href = discover_constants_href().await?;
    let bytes = http::get(&href).await?;
    let lats = coord_field(&bytes, LAT_ID)?;
    let lons = coord_field(&bytes, LON_ID)?;
    if lats.len() != lons.len() {
        return None;
    }
    platform::write_bytes(&lat_path, &f32_to_bytes(&lats));
    platform::write_bytes(&lon_path, &f32_to_bytes(&lons));
    Some((lats, lons))
}

/// Reinterpret a byte slice as little-endian f32 (length not a multiple of 4 is
/// truncated).
fn f32_from_bytes(b: &[u8]) -> Vec<f32> {
    b.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

/// Serialize f32 values to little-endian bytes.
fn f32_to_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Decode the single wind field message to per-cell values in knots.
fn field_kn(bytes: &[u8]) -> Option<Vec<f32>> {
    let m = read_messages(bytes).next()?;
    let d = m.data().ok()?;
    Some(d.into_iter().map(|x| x as f32 * MS_TO_KN).collect())
}

/// Load ICON-CH1 as a regular Swiss `WindGrid`: fetch (cached) per-cell coords,
/// fetch the latest control u/v, regrid the mesh onto `BOX` at `STEP`. None if
/// discovery, fetch, or decode fails.
pub async fn load_grid(cache_location: &str) -> Option<WindGrid> {
    let (lats, lons) = cell_coords(cache_location).await?;

    let (u_href, v_href) = discover_wind_hrefs().await?;
    // cache the raw grib per host filename (which encodes the run + step).
    let u_bytes = fetch_cached(cache_location, &u_href).await?;
    let v_bytes = fetch_cached(cache_location, &v_href).await?;
    let u = field_kn(&u_bytes)?;
    let v = field_kn(&v_bytes)?;
    if u.len() != lats.len() || v.len() != lats.len() {
        return None;
    }
    Some(WindGrid::from_scattered(&lats, &lons, &u, &v, BOX, STEP))
}

/// GET a signed href, caching the bytes under the object filename (which encodes
/// the run and variable, e.g. `icon-ch1-eps-202608081500-0-u_10m-ctrl.grib2`).
async fn fetch_cached(cache_location: &str, href: &str) -> Option<Vec<u8>> {
    let name = href.rsplit('/').next()?.split('?').next()?;
    let path = format!("{cache_location}/wind/{name}");
    if let Some(b) = platform::read_bytes(&path) {
        return Some(b);
    }
    let b = http::get(href).await?;
    platform::write_bytes(&path, &b);
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_control_title_fragment() {
        assert_eq!(
            title_fragment("2026-08-08T15:00:00Z", "U_10M").unwrap(),
            "U_10M at 08.08.2026 15:00 Step 0 (Control)"
        );
        assert!(title_fragment("garbage", "U_10M").is_none());
    }
}
