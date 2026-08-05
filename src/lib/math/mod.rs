mod screen;
mod tile_field;
mod tile_id;

use lyon::math::{Point, point, vector};
use std::f32::consts::{E, PI};

pub use screen::*;
pub use tile_field::*;
pub use tile_id::*;

pub type Vector2 = nalgebra::Vector2<f32>;
pub type Point2 = nalgebra::Point2<f32>;
pub type Rotation2 = nalgebra::Rotation2<f32>;

pub trait EuclidVsNalgebra {
    type Value;
    fn convert(self) -> Self::Value;
}

impl EuclidVsNalgebra for lyon::math::Point {
    type Value = Point2;

    fn convert(self) -> Self::Value {
        Point2::new(self.x, self.y)
    }
}

impl EuclidVsNalgebra for Point2 {
    type Value = lyon::math::Point;

    fn convert(self) -> Self::Value {
        point(self.x, self.y)
    }
}

impl EuclidVsNalgebra for Vector2 {
    type Value = lyon::math::Vector;

    fn convert(self) -> Self::Value {
        vector(self.x, self.y)
    }
}

/// Converts radians to degrees.
const fn rad2deg(rad: f32) -> f32 {
    rad * 360.0 / (2.0 * PI)
}

/// Converts degrees to radians.
const fn deg2rad(deg: f32) -> f32 {
    2.0 * PI * deg / 360.0
}

/// Converts latitude and longitude into euclidian x and y coordinates.
///
/// Factors in the current tile zoom level and returns a tile coordinate within a grid of 2^zoom tiles in width.
///
/// This reresents the inverse Mercator projection.
pub fn deg2num(lat_deg: f32, lon_deg: f32, zoom: u32) -> TileCoordinate {
    let lat_rad = deg2rad(lat_deg);
    let n = f32::powi(2.0, zoom as i32);
    let xtile = (lon_deg + 180.0) / 360.0 * n;
    let ytile = (1.0 - (PI / 4.0 + lat_rad / 2.0).tan().ln() / PI) / 2.0 * n;

    TileCoordinate::new(zoom, xtile, ytile)
}

/// Converts euclidian x and y coordinates into latitude and longitude.
///
/// This is the Mercator projection and the inverse of [`deg2num`].
pub fn num2deg(tile: TileCoordinate) -> Point {
    let n = f32::powi(2.0, tile.z as i32);

    let lon_deg = tile.x * 360.0 / n - 180.0;
    let lat_rad = 2.0 * (E.powf(1.0 - tile.y * 2.0 / n).atan() * PI - PI / 4.0);
    let lat_deg = rad2deg(lat_rad);

    point(lon_deg, lat_deg)
}

pub fn tile_to_world_space(coordinate: &TileCoordinate) -> Point {
    point(0.0, 0.0) + vector(coordinate.x, coordinate.y) * f32::powi(2.0, -(coordinate.z as i32))
}

pub fn world_to_tile_space(point: &Point, z: u32) -> TileCoordinate {
    let p = *point * 2f32.powi(z as i32);
    TileCoordinate::new(z, p.x, p.y)
}
