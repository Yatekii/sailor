use std::path::Path;
use std::sync::OnceLock;

use crate::platform;
use sailor_math::math::TileId;

/// One shared client for all tile fetches. `reqwest::get` builds a fresh Client
/// per call (new TCP + TLS handshake, no keep-alive, no HTTP/2 multiplexing) —
/// reusing one pools connections and is dramatically faster when panning.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

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
        // OSM US public OpenMapTiles endpoint (unpadded z/x/y, same OMT schema).
        None => format!(
            "https://tiles.openstreetmap.us/vector/openmaptiles/{}/{}/{}.mvt",
            tile_id.z, tile_id.x, tile_id.y
        ),
    };

    match client().get(&request_url).send().await {
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
