mod screen;
mod tile_field;
mod tile_id;

use lyon::math::{point, vector, Point};
use std::f32::consts::PI;

pub use screen::*;
pub use tile_field::*;
pub use tile_id::*;

fn deg2rad(deg: f32) -> f32 {
    2.0 * PI * deg / 360.0
}

pub fn deg2num(lat_deg: f32, lon_deg: f32, zoom: u32) -> TileCoordinate {
    let lat_rad = deg2rad(lat_deg);
    let n = 2f32.powi(zoom as i32);
    let xtile = (lon_deg + 180.0) / 360.0 * n;
    let ytile = (1.0 - (lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI) / 2.0 * n;

    TileCoordinate::new(zoom, xtile, ytile)
}

pub fn num_to_global_space(coordinate: &TileCoordinate) -> Point {
    point(0.0, 0.0) + vector(coordinate.x, coordinate.y) * 1.0 / 2f32.powi(coordinate.z as i32)
}

pub fn global_to_num_space(point: &Point, z: u32) -> TileCoordinate {
    let p = *point / 2f32.powi(-(z as i32));
    TileCoordinate::new(z, p.x, p.y)
}
