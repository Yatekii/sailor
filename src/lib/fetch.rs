use std::{fs::File, io::Read, path::Path};

use super::*;

pub fn fetch_tile_data(cache_location: impl AsRef<Path>, tile_id: &TileId) -> Option<Vec<u8>> {
    let zxy: String = format!("{}", tile_id);
    let pbf = format!("cache/{}.pbf", zxy);
    if !is_in_cache(pbf.clone()) {
        if let Some(data) = fetch_tile_from_server(tile_id) {
            ensure_cache_structure(cache_location, tile_id);
            match File::create(&pbf) {
                Ok(mut file) => {
                    use std::io::Write;
                    match file.write_all(&data[..]) {
                        Ok(_) => Some(data),
                        Err(e) => {
                            log::error!("Unable to write pbf {}. Reason:\r\n{}", pbf, e);
                            None
                        }
                    }
                }
                Err(e) => {
                    log::error!("Could not create pbf {}. Reason:\r\n{}", pbf, e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        match File::open(&pbf) {
            Ok(mut f) => {
                let mut buffer = Vec::new();
                match f.read_to_end(&mut buffer) {
                    Ok(_) => Some(buffer),
                    Err(e) => {
                        log::error!("Unable to read {}. Reason:\r\n{}", pbf, e);
                        None
                    }
                }
            }
            Err(e) => {
                log::error!("Unable to open {}. Reason:\r\n{}", pbf, e);
                None
            }
        }
    }
}

fn fetch_tile_from_server(tile_id: &TileId) -> Option<Vec<u8>> {
    let request_url = format!(
        "https://api.maptiler.com/tiles/v3/{}.pbf?key=t2mP0OQnprAXkW20R6Wd",
        tile_id
    );
    let response = ureq::get(&request_url).call();
    if response.ok() {
        let mut reader = response.into_reader();
        let mut data = vec![];
        match reader.read_to_end(&mut data) {
            Ok(_) => Some(data),
            Err(e) => {
                log::warn!(
                    "Could not read http response for {} to buffer. Reason:\r\n{}",
                    tile_id,
                    e
                );
                None
            }
        }
    } else {
        log::warn!(
            "Http request for {} failed. Reason:\r\n{:?}",
            tile_id,
            response.status()
        );
        None
    }
}

fn is_in_cache(path: impl Into<String>) -> bool {
    Path::new(&path.into()).exists()
}

fn ensure_cache_structure(root: impl AsRef<Path>, tile_id: &TileId) {
    let dir_path = root
        .as_ref()
        .join(&format!("cache/{}/{}/", tile_id.z, tile_id.x));
    std::fs::create_dir_all(dir_path).expect("Could not create cache directories.");
}

#[test]
fn test_ensure_cache_structure() {
    ensure_cache_structure("/tmp/sailor-test", &crate::TileId::new(8, 42, 42));
    let md = std::fs::metadata("/tmp/sailor-test/cache/8/42");
    assert!(md.is_ok());
    assert!(md.unwrap().is_dir());
}
