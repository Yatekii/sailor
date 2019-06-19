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

pub fn deg2tile(lat_deg: f32, lon_deg: f32, zoom: u32) -> TileId {
    deg2num(lat_deg, lon_deg, zoom).into()
}

pub fn deg2num(lat_deg: f32, lon_deg: f32, zoom: u32) -> TileCoordinate {
    let lat_rad = deg2rad(lat_deg);
    let n = 2f32.powi(zoom as i32);
    let xtile = (lon_deg + 180.0) / 360.0 * n;
    let ytile = (
        1.0 - (
            lat_rad.tan() + 1.0 / lat_rad.cos()
        ).ln() / PI
    ) / 2.0 * n;

    TileCoordinate::new(zoom, xtile, ytile)
}

// pub fn num2deg(xtile: u32, ytile: u32, zoom: u32) -> (f32, f32) {
//     let n = 2f32.powi(zoom as i32);
//     let lon_deg = xtile as f32 / n * 360.0 - 180.0;
//     let lat_rad = ((PI * (1f32 - 2f32 * ytile as f32 / n)).sinh()).atan();
//     let lat_deg = rad2deg(lat_rad);
//     (lat_deg, lon_deg)
// }

pub fn tile_to_global_space(coordinate: &TileId, point: Point) -> Point {
    (point + vector(coordinate.x as f32, coordinate.y as f32)) * 1.0/2f32.powi(coordinate.z as i32)
}

pub fn num_to_global_space(coordinate: &TileCoordinate) -> Point {
    point(0.0, 0.0) + vector(coordinate.x, coordinate.y) * 1.0/2f32.powi(coordinate.z as i32)
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

    pub fn get_tile_boundaries_for_zoom_level(&self, z: u32) -> (TileId, TileId) {
        (deg2tile(self.topleft.x, self.topleft.y, z), deg2tile(self.bottomright.x, self.bottomright.y, z))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TileId {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn new(z: u32, x: u32, y: u32) -> Self {
        Self {
            z, x, y
        }
    }
}

impl From<TileCoordinate> for TileId {
    fn from(value: TileCoordinate) -> Self {
        Self {
            z: value.z,
            x: value.x as u32,
            y: value.y as u32,
        }
    }
}

impl std::fmt::Display for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    } 
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct TileCoordinate {
    pub z: u32,
    pub x: f32,
    pub y: f32,
}

impl TileCoordinate {
    pub fn new(z: u32, x: f32, y: f32) -> Self {
        Self {
            z, x, y
        }
    }
}

impl From<TileId> for TileCoordinate {
    fn from(value: TileId) -> Self {
        Self {
            z: value.z,
            x: value.x as f32,
            y: value.y as f32,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TileField {
    pub topleft: TileId,
    pub bottomright: TileId,
}

impl TileField {
    pub fn new(topleft: TileId, bottomright: TileId) -> Self {
        Self {
            topleft,
            bottomright,
        }
    }
}

#[test]
fn get_tile_boundaries_for_8_zoom() {
    let bb = BoundingBox::new(point(47.607371, 6.114297), point(46.047108, 10.212341));
    dbg!(bb.get_tile_boundaries_for_zoom_level(8));
}