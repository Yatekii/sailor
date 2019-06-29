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

// pub fn deg2tile(lat_deg: f32, lon_deg: f32, zoom: u32) -> TileId {
//     deg2num(lat_deg, lon_deg, zoom).into()
// }

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

// pub fn num2deg(tile_coordinate: &TileCoordinate) -> Point {
//     let n = 2f32.powi(tile_coordinate.z as i32);
//     let lon_deg = tile_coordinate.x as f32 / n * 360.0 - 180.0;
//     let lat_rad = ((PI * (1f32 - 2f32 * tile_coordinate.y as f32 / n)).sinh()).atan();
//     let lat_deg = rad2deg(lat_rad);
//     point(lat_deg, lon_deg)
// }

// pub fn tile2deg(tile_coordinate: &TileId) -> Point {
//     num2deg(&tile_coordinate.clone().into())
// }

pub fn tile_to_global_space(coordinate: &TileId, point: Point) -> Point {
    (point + vector(coordinate.x as f32, coordinate.y as f32)) * 1.0/2f32.powi(coordinate.z as i32)
}

pub fn num_to_global_space(coordinate: &TileCoordinate) -> Point {
    point(0.0, 0.0) + vector(coordinate.x, coordinate.y) * 1.0/2f32.powi(coordinate.z as i32)
}

pub fn global_to_num_space(point: &Point, z: u32) -> TileCoordinate {
    let p = *point / 2f32.powi(-(z as i32));
    TileCoordinate::new(z, p.x, p.y)
}

// pub fn global_to_tile_space(z: u32, x: u32, y: u32, point: Point) -> Point {
//     point - vector(x as f32, y as f32) * 2f32.powi(z as i32)
// }

pub struct Screen {
    pub center: Point,
    pub width: u32,
    pub height: u32,
}

impl Screen {
    pub fn new(center: Point, width: u32, height: u32) -> Self {
        Self {
            center,
            width,
            height,
        }
    }

    pub fn get_tile_boundaries_for_zoom_level(&self, z: f32) -> TileField {
        let z = z.min(14.0);
        let px_to_world = self.width as f32 / 256.0 / 2.0;
        let py_to_world = self.height as f32 / 256.0 / 2.0;

        let middle_tile: TileId = global_to_num_space(&self.center, z as u32).into();
        TileField::new(
            middle_tile - TileId::new(
                z as u32,
                px_to_world.ceil().min(middle_tile.x as f32) as u32,
                py_to_world.ceil().min(middle_tile.y as f32) as u32
            ),
            middle_tile + TileId::new(
                z as u32,
                px_to_world.ceil().min(2f32.powi(z as i32) - middle_tile.x as f32) as u32,
                py_to_world.ceil().min(2f32.powi(z as i32) - middle_tile.y as f32) as u32
            )
        )
    }

    pub fn global_to_screen(&self, z: f32) -> glm::TMat4<f32> {
        let zoom_x = 2.0f32.powf(z) / (self.width as f32 / 2.0) * 256.0;
        let zoom_y = 2.0f32.powf(z) / (self.height as f32 / 2.0) * 256.0;
        let zoom = glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0));
        let position = glm::translation(&glm::vec3(-self.center.x, -self.center.y, 0.0));
        glm::transpose(&(zoom * position))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
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

impl std::ops::Add for TileId {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        assert_eq!(self.z, other.z);
        Self {
            z: self.z,
            x: self.x + other.x,
            y: self.y + other.y,
        }
    }
}

impl std::ops::AddAssign for TileId {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::Sub for TileId {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        assert_eq!(self.z, other.z);
        Self {
            z: self.z,
            x: self.x - other.x,
            y: self.y - other.y,
        }
    }
}

impl std::ops::SubAssign for TileId {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
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

    pub fn iter<'a>(&'a self) -> TileIterator<'a> {
        TileIterator {
            tile_field: self,
            current_tile: self.topleft,
        }
    }
}

pub struct TileIterator<'a> {
    tile_field: &'a TileField,
    current_tile: TileId,
}

impl<'a> Iterator for TileIterator<'a> {
    type Item = TileId;

    fn next(&mut self) -> Option<Self::Item> {
        for _ in self.current_tile.x..self.tile_field.bottomright.x + 1 {
            let c = self.current_tile;
            self.current_tile = self.current_tile + TileId::new(self.current_tile.z, 1, 0);
            return Some(c)
        }
        if self.current_tile.y < self.tile_field.bottomright.y {
            self.current_tile = TileId::new(self.current_tile.z, self.tile_field.topleft.x + 1, self.current_tile.y + 1);
            let c = self.current_tile - TileId::new(self.current_tile.z, 1, 0);
            Some(c)
        } else {
            None
        }
    }
}

#[test]
fn get_tile_boundaries_for_8_zoom() {
    let bb = Screen::new(point(47.607371, 6.114297), 800, 800);
    let tile_field = bb.get_tile_boundaries_for_zoom_level(8.0);
    let tiles = tile_field.iter().collect::<Vec<_>>();

    assert_eq!(tiles.len(), 25);
}