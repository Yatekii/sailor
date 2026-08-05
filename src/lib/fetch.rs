use std::path::Path;

use crate::math::TileId;

/// Fetch the raw vector-tile bytes for a tile.
///
/// Natively a local disk cache is consulted first; on the web the tile is always
/// fetched from the network (the browser does its own HTTP caching).
pub async fn fetch_tile_data(cache_location: &Path, tile_id: &TileId) -> Option<Vec<u8>> {
    if let Some(cached) = read_from_cache(tile_id) {
        return Some(cached);
    }

    let data = fetch_tile_from_server(tile_id).await?;
    write_to_cache(cache_location, tile_id, &data);
    Some(data)
}

/// Fetch a tile from the tile server. reqwest is async on both native (tokio) and
/// web (browser fetch), so this is a single cross-platform implementation.
async fn fetch_tile_from_server(tile_id: &TileId) -> Option<Vec<u8>> {
    let request_url = format!("https://d17gef4m69t9r4.cloudfront.net/planet/{tile_id}.mvt");

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

#[cfg(not(target_arch = "wasm32"))]
fn read_from_cache(tile_id: &TileId) -> Option<Vec<u8>> {
    use std::io::Read;
    let pbf = format!("cache/{tile_id}.pbf");
    if !Path::new(&pbf).exists() {
        return None;
    }
    match std::fs::File::open(&pbf) {
        Ok(mut f) => {
            let mut buffer = Vec::new();
            match f.read_to_end(&mut buffer) {
                Ok(_) => Some(buffer),
                Err(e) => {
                    log::error!("Unable to read {pbf}. Reason:\r\n{e}");
                    None
                }
            }
        }
        Err(e) => {
            log::error!("Unable to open {pbf}. Reason:\r\n{e}");
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_to_cache(cache_location: &Path, tile_id: &TileId, data: &[u8]) {
    use std::io::Write;
    ensure_cache_structure(cache_location, tile_id);
    let pbf = format!("cache/{tile_id}.pbf");
    match std::fs::File::create(&pbf) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(data) {
                log::error!("Unable to write pbf {pbf}. Reason:\r\n{e}");
            }
        }
        Err(e) => log::error!("Could not create pbf {pbf}. Reason:\r\n{e}"),
    }
}

/// Creates all necessary directories on disk to store the PBF file of the given `tile_id`.
#[cfg(not(target_arch = "wasm32"))]
fn ensure_cache_structure(root: &Path, tile_id: &TileId) {
    let dir_path = root.join(format!("cache/{:0>3}/{:0>3}/", tile_id.z, tile_id.x));
    std::fs::create_dir_all(dir_path).expect("Could not create cache directories.");
}

// The web has no filesystem cache; the browser caches HTTP responses itself.
#[cfg(target_arch = "wasm32")]
fn read_from_cache(_tile_id: &TileId) -> Option<Vec<u8>> {
    None
}

#[cfg(target_arch = "wasm32")]
fn write_to_cache(_cache_location: &Path, _tile_id: &TileId, _data: &[u8]) {}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn test_ensure_cache_structure() {
    ensure_cache_structure(Path::new("/tmp/sailor-test"), &TileId::new(8, 42, 42));
    let md = std::fs::metadata("/tmp/sailor-test/cache/008/042");
    assert!(md.is_ok());
    assert!(md.unwrap().is_dir());
}
