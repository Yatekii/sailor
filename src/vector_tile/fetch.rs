use std::fs::File;
use std::path::Path;
use std::io::Read;
use crate::vector_tile::math;

pub fn fetch_tile_data(tile_id: &math::TileId) -> Option<Vec<u8>> {
    let zxy: String = format!("{}", tile_id);
    let pbf = format!("cache/{}.pbf", zxy);
    if !is_in_cache(pbf.clone()) {
        if let Some(data) = fetch_tile_from_server(tile_id) {
            ensure_cache_structure(tile_id);
            match File::create(&pbf) {
                Ok(mut file) => {
                    use std::io::Write;
                    match file.write_all(&data[..]) {
                        Ok(_) => {
                            Some(data)
                        },
                        Err(e) => {
                            log::error!("Unable to write pbf {}. Reason:\r\n{}", pbf, e);
                            None
                        },
                    }
                },
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
                    Ok(_) => {
                        Some(buffer)
                    },
                    Err(e) => {
                        log::error!("Unable to read {}. Reason:\r\n{}", pbf, e);
                        None
                    },
                }
            },
            Err(e) => {
                log::error!("Unable to open {}. Reason:\r\n{}", pbf, e);
                None
            }
        }
    }
}

fn fetch_tile_from_server(tile_id: &math::TileId) -> Option<Vec<u8>> {
    let request_url = format!("https://api.maptiler.com/tiles/v3/{}.pbf?key=t2mP0OQnprAXkW20R6Wd", tile_id);
    match reqwest::get(&request_url) {
        Ok(mut resp) => {
            if resp.status() != reqwest::StatusCode::OK {
                log::warn!("Tile request failed for {}. Status was:\r\n{}", tile_id, resp.status());
                None
            } else {
                let mut data: Vec<u8> = vec![];
                match resp.copy_to(&mut data) {
                    Ok(_) => {
                        Some(data)
                    },
                    Err(e) => {
                        log::warn!("Could not read http response for {} to buffer. Reason:\r\n{}", tile_id, e);
                        None
                    },
                }
            }
        },
        Err(err) => {
            log::warn!("Http request for {} failed. Reason:\r\n{:?}", tile_id, err);
            None
        }
    }
}

fn is_in_cache(path: impl Into<String>) -> bool {
    Path::new(&path.into()).exists()
}

fn ensure_cache_structure(tile_id: &math::TileId) {
    let dir_path = format!("cache/{}/{}/", tile_id.z, tile_id.x);
    std::fs::create_dir_all(dir_path).expect("Could not create cache directories.");
}