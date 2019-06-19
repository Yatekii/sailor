use std::fs::File;
use std::path::Path;
use std::io::Read;
use crate::vector_tile::math;

pub fn fetch_tile_data(tile_id: &math::TileId) -> Vec<u8> {
    let zxy: String = format!("{}", tile_id);
    let pbf = format!("cache/{}.pbf", zxy);
    if !is_in_cache(pbf.clone()) {
        let data = fetch_tile_from_server(tile_id);
        ensure_cache_structure(tile_id);
        let mut file = File::create(&pbf).expect("Could not create pbf file.");

        use std::io::Write;
        file.write_all(&data[..]).expect("Could not write bytes.");
        data
    } else {
        let mut f = File::open(pbf).expect("Unable to open file.");
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer).expect("Unable to read file.");
        buffer
    }
}

fn fetch_tile_from_server(tile_id: &math::TileId) -> Vec<u8> {
    let request_url = format!("https://api.maptiler.com/tiles/v3/{}.pbf?key=t2mP0OQnprAXkW20R6Wd", tile_id);
    let mut resp = reqwest::get(&request_url).expect("Could not load tile.");
    if resp.status() != reqwest::StatusCode::OK {
        panic!("Tile request failed.");
    }
    let mut data: Vec<u8> = vec![];
    resp.copy_to(&mut data).expect("Could not read http response to buffer.");
    data
}

fn is_in_cache(path: impl Into<String>) -> bool {
    Path::new(&path.into()).exists()
}

fn ensure_cache_structure(tile_id: &math::TileId) {
    let dir_path = format!("cache/{}/{}/", tile_id.z, tile_id.x);
    std::fs::create_dir_all(dir_path).expect("Could not create cache directories.");
}