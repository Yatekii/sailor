pub use super::*;

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

    pub fn iter(&self) -> TileIterator {
        TileIterator {
            tile_field: self,
            current_tile: self.topleft,
        }
    }

    pub fn contains(&self, tile_id: &TileId) -> bool {
        for tile in self.iter() {
            if &tile == tile_id {
                return true;
            }
        }
        false
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
            // TODO error?
            //    = note: `#[warn(clippy::never_loop)]` on by default
            //    = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#never_loop
            return Some(c);
        }
        if self.current_tile.y < self.tile_field.bottomright.y {
            self.current_tile = TileId::new(
                self.current_tile.z,
                self.tile_field.topleft.x + 1,
                self.current_tile.y + 1,
            );
            let c = self.current_tile - TileId::new(self.current_tile.z, 1, 0);
            Some(c)
        } else {
            None
        }
    }
}

#[test]
fn get_tile_boundaries_for_8_zoom() {
    use super::*;
    let bb = Screen::new(point(47.607_372, 6.114297), 800, 800, 256, 1.0);
    let tile_field = bb.get_tile_boundaries_for_zoom_level(8.0, 1);

    assert_eq!(tile_field.iter().count(), 20);
}
