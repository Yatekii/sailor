use std::fs::File;
use std::path::Path;
use std::io::Read;

pub fn fetch_tile_data(z: u32, x: u32, y: u32) -> Vec<u8> {
    let zxy: String = format!("/{}/{}/{}", z, x, y);
    let pbf = format!("cache{}.pbf", zxy);
    if !is_in_cache(pbf.clone()) {
        let data = fetch_tile_from_server(z, x, y);
        ensure_cache_structure(z, x);
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

fn fetch_tile_from_server(z: u32, x: u32, y: u32) -> Vec<u8> {
    let request_url = format!("https://api.maptiler.com/tiles/v3/{}/{}/{}.pbf?key=t2mP0OQnprAXkW20R6Wd", z, x, y);
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

fn ensure_cache_structure(z: u32, x: u32) {
    let dir_path = format!("cache/{}/{}/", z, x);
    std::fs::create_dir_all(dir_path).expect("Could not create cache directories.");
}