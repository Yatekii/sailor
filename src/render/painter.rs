use lyon::math::Point;
use crate::vector_tile::{
    math,
    cache::{
        Tile,
        TileCache,
    },
};
use crate::render::layer::RenderLayer;
use crate::render::css::RulesCache;

pub struct Painter<'a> {
    render_layers: Vec<RenderLayer>,
    tile_field: math::TileField,
    display: &'a glium::Display,
    program: &'a glium::Program,
}

impl<'a> Painter<'a> {
    pub fn new(display: &'a glium::Display, program: &'a glium::Program) -> Self {
        Self {
            render_layers: vec![],
            tile_field: math::TileField::new(math::TileId::new(8, 0, 0), math::TileId::new(8, 0, 0)),
            display,
            program,
        }
    }

    pub fn paint(&mut self, cache: &mut TileCache, css_cache: &mut RulesCache, screen: &math::Screen, z: u32, pan: Point) {
        let tile_field = screen.get_tile_boundaries_for_zoom_level(z);

        use crate::glium::Surface;
        let mut target = self.display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);

        if self.tile_field != tile_field {
            self.tile_field = tile_field;
            cache.fetch_tiles(screen);
            self.render_layers = cache
                .get_tiles(screen)
                .into_iter()
                .flat_map(|tile| tile.layers.into_iter().map(|layer| RenderLayer::new(layer.with_style(css_cache), &self.display)))
                .collect::<Vec<_>>();
            dbg!(&self.render_layers.len());
        }
        for rl in &mut self.render_layers {
            if css_cache.update() {
                println!("Cache update");
                dbg!(&rl.layer.color);
                take_mut::take(&mut rl.layer, |layer| layer.with_style(css_cache));
                dbg!(&rl.layer.color);
            }
            rl.draw(&mut target, &mut self.program, pan * -1.0);
        }

        target.finish().unwrap();
    }
}