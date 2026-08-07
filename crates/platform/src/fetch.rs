use std::path::Path;

use crate::platform;
use sailor_math::math::TileId;

/// Fetch the raw vector-tile bytes for a tile.
///
/// A local cache is consulted first (a no-op on the web, where the browser caches
/// HTTP responses itself), otherwise the tile is fetched from the tile server.
pub async fn fetch_tile_data(cache_location: &Path, tile_id: &TileId) -> Option<Vec<u8>> {
    let pbf = cache_location.join(format!("{tile_id}.pbf"));
    let pbf = pbf.to_string_lossy();

    if let Some(cached) = platform::read_bytes(&pbf) {
        return Some(cached);
    }

    let data = fetch_tile_from_server(tile_id).await?;
    platform::write_bytes(&pbf, &data);
    Some(data)
}

/// Fetch a tile from the tile server. reqwest is async on both native (tokio) and
/// web (browser fetch), so this is a single cross-platform implementation.
async fn fetch_tile_from_server(tile_id: &TileId) -> Option<Vec<u8>> {
    // On the web the tiles are fetched from a same-origin path that the dev
    // server proxies to the CDN, so the browser does not block them with CORS.
    let request_url = match platform::origin() {
        Some(origin) => format!("{origin}/planet/{tile_id}.mvt"),
        None => format!("https://d17gef4m69t9r4.cloudfront.net/planet/{tile_id}.mvt"),
    };

    match reqwest::get(&request_url).await {
        Ok(response) if response.status().is_success() => match response.bytes().await {
            Ok(bytes) => Some(bytes.to_vec()),
            Err(e) => {
                log::warn!("Could not read http response for {tile_id}. Reason:\r\n{e}");
                None
            }
        },
        Ok(response) => {
            log::warn!("Http request for {tile_id} failed: {}", response.status());
            None
        }
        Err(e) => {
            log::warn!("Http request for {tile_id} errored: {e}");
            None
        }
    }
}
