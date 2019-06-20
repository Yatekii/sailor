use lyon::math::Point;
use crate::vector_tile::{
    math,
    cache::{
        Tile,
        TileCache,
    },
};

pub struct Painter<'a> {
    tiles: Vec<Tile>,
    tile_field: math::TileField,
    display: &'a glium::Display,
    program: &'a glium::Program,
}

impl<'a> Painter<'a> {
    pub fn new(display: &'a glium::Display, program: &'a glium::Program) -> Self {
        Self {
            tiles: vec![],
            tile_field: math::TileField::new(math::TileId::new(8, 0, 0), math::TileId::new(8, 0, 0)),
            display,
            program,
        }
    }

    pub fn paint(&mut self, cache: &mut TileCache, screen: &math::Screen, z: u32, pan: Point) {
        let tile_field = screen.get_tile_boundaries_for_zoom_level(z);

        use crate::glium::Surface;
        let mut target = self.display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);

        if self.tile_field != tile_field {
            self.tile_field = tile_field;
            cache.fetch_tiles(screen);
            self.tiles = cache.get_tiles(screen);

            for tile in &mut self.tiles {
                for layer in &mut tile.layers {
                    layer.load(&mut self.display);
                    layer.draw(&mut target, &mut self.program, pan * -1.0);
                }
            }
        } else {
            for tile in &mut self.tiles {
                for layer in &mut tile.layers {
                    layer.draw(&mut target, &mut self.program, pan * -1.0);
                }
            }
        }

        target.finish().unwrap();
    }
}