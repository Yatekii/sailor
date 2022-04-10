#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct TileId {
    pub z: u32,
    pub x: u32,
    pub y: u32,
}

impl TileId {
    pub fn new(z: u32, x: u32, y: u32) -> Self {
        Self { z, x, y }
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
        write!(f, "{:0>3}/{:0>3}/{:0>3}", self.z, self.x, self.y)
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
        Self { z, x, y }
    }
}
