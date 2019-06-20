fn main() {
    let mut cache = crate::vector_tile::cache::TileCache::new();
    let z = 8;
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z);
    let zurich = math::num_to_global_space(&tile_coordinate);
    let mut screen = math::Screen::new(zurich, 600, 600);

    cache.fetch_tiles(&screen);
}