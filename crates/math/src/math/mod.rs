mod camera;
mod space;
mod tile_field;
mod tile_id;

use lyon::math::{Point, point, vector};
use std::f32::consts::PI;

pub use camera::*;
pub use space::*;
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
pub fn deg2num(coord: Coord<Geo>, zoom: u32) -> TileCoordinate {
    let lat_rad = deg2rad(coord.y());
    let n = f32::powi(2.0, zoom as i32);
    let xtile = (coord.x() + 180.0) / 360.0 * n;
    let ytile = (1.0 - (PI / 4.0 + lat_rad / 2.0).tan().ln() / PI) / 2.0 * n;

    TileCoordinate::new(zoom, xtile, ytile)
}

/// Converts euclidian x and y coordinates into latitude and longitude.
///
/// This is the Mercator projection and the inverse of [`deg2num`].
pub fn num2deg(tile: TileCoordinate) -> Coord<Geo> {
    let n = f32::powi(2.0, tile.z as i32);

    let lon_deg = tile.x * 360.0 / n - 180.0;
    // Inverse of deg2num's y: phi = 2*(atan(e^(pi*(1 - 2y/n))) - pi/4).
    let lat_rad = 2.0 * ((PI * (1.0 - tile.y * 2.0 / n)).exp().atan() - PI / 4.0);
    let lat_deg = rad2deg(lat_rad);

    Coord::<Geo>::new(lon_deg, lat_deg)
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    fn approx(a: f32, b: f32, eps: f32) {
        assert!((a - b).abs() < eps, "{a} != {b}");
    }

    // (0,0) lat/lon sits at the middle of the grid: x = y = 2^z / 2.
    #[test]
    fn origin_maps_to_grid_center() {
        for z in [0u32, 4, 8, 14] {
            let t = deg2num(Coord::<Geo>::new(0.0, 0.0), z);
            let mid = 2f32.powi(z as i32) / 2.0;
            approx(t.x, mid, 1e-3);
            approx(t.y, mid, 1e-3);
        }
    }

    #[test]
    fn deg_num_roundtrips() {
        let z = 12;
        let original = Coord::<Geo>::new(6.114297, 47.607372); // lon, lat
        let back = num2deg(deg2num(original, z));
        approx(back.x(), original.x(), 1e-3);
        approx(back.y(), original.y(), 1e-3);
    }
}

pub fn tile_to_world_space(coordinate: &TileCoordinate) -> Point {
    point(0.0, 0.0) + vector(coordinate.x, coordinate.y) * f32::powi(2.0, -(coordinate.z as i32))
}

pub fn world_to_tile_space(point: &Point, z: u32) -> TileCoordinate {
    let p = *point * 2f32.powi(z as i32);
    TileCoordinate::new(z, p.x, p.y)
}
