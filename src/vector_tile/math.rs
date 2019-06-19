use std::f32::consts::PI;
use lyon::math::{
    Point,
    point,
    vector,
};

fn deg2rad(deg: f32) -> f32 {
    2.0 * PI * deg / 360.0
}

// fn rad2deg(rad: f32) -> f32 {
//     360.0 * rad / (2.0 * PI)
// }

pub fn deg2tile(lat_deg: f32, lon_deg: f32, zoom: u32) -> (u32, u32) {
    let point = deg2num(lat_deg, lon_deg, zoom);

    (point.x as u32, point.y as u32)
}

pub fn deg2num(lat_deg: f32, lon_deg: f32, zoom: u32) -> Point {
    let lat_rad = deg2rad(lat_deg);
    let n = 2f32.powi(zoom as i32);
    let xtile = (lon_deg + 180.0) / 360.0 * n;
    let ytile = (
        1.0 - (
            lat_rad.tan() + 1.0 / lat_rad.cos()
        ).ln() / PI
    ) / 2.0 * n;

    point(xtile, ytile)
}

// pub fn num2deg(xtile: u32, ytile: u32, zoom: u32) -> (f32, f32) {
//     let n = 2f32.powi(zoom as i32);
//     let lon_deg = xtile as f32 / n * 360.0 - 180.0;
//     let lat_rad = ((PI * (1f32 - 2f32 * ytile as f32 / n)).sinh()).atan();
//     let lat_deg = rad2deg(lat_rad);
//     (lat_deg, lon_deg)
// }

pub fn tile_to_global_space(z: u32, x: u32, y: u32, point: Point) -> Point {
    (point + vector(x as f32, y as f32) * 2f32.powi(z as i32)) / 4.0;
    point / 4096.0
}

pub fn num_to_global_space(z: u32, x: f32, y: f32) -> Point {
    (point(0.0, 0.0) + vector(x, y) * 2f32.powi(z as i32)) / 4.0
}

// pub fn global_to_tile_space(z: u32, x: u32, y: u32, point: Point) -> Point {
//     point - vector(x as f32, y as f32) * 2f32.powi(z as i32)
// }

pub struct BoundingBox {
    topleft: Point,
    bottomright: Point,
}

impl BoundingBox {
    pub fn new(topleft: Point, bottomright: Point) -> Self {
        Self {
            topleft,
            bottomright,
        }
    }

    pub fn get_tile_boundaries_for_zoom_level(&self, z: u32) -> ((u32, u32), (u32, u32)) {
        (deg2tile(self.topleft.x, self.topleft.y, z), deg2tile(self.bottomright.x, self.bottomright.y, z))
    }
}

#[test]
fn get_tile_boundaries_for_8_zoom() {
    let bb = BoundingBox::new(point(47.607371, 6.114297), point(46.047108, 10.212341));
    dbg!(bb.get_tile_boundaries_for_zoom_level(8));
}