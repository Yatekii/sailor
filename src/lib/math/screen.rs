use super::*;

pub struct Screen {
    pub center: Point,
    pub width: u32,
    pub height: u32,
    tile_size: u32,
}

impl Screen {
    pub fn new(center: Point, width: u32, height: u32, tile_size: u32, hidpi_factor: f64) -> Self {
        Self {
            center,
            width,
            height,
            tile_size: (tile_size as f64 * hidpi_factor) as u32,
        }
    }

    pub fn get_tile_size(&self) -> u32 {
        self.tile_size
    }

    pub fn get_tile_boundaries_for_zoom_level(&self, z: f32, scale: u32) -> TileField {
        let z = z.min(14.0);
        let px_to_world = self.width as f32
            / self.get_tile_size() as f32
            / 2.0
            / 2f32.powi(z as i32)
            / scale as f32;
        let py_to_world = self.height as f32
            / self.get_tile_size() as f32
            / 2.0
            / 2f32.powi(z as i32)
            / scale as f32;

        let top_left: TileId =
            global_to_num_space(&(self.center - vector(px_to_world, py_to_world)), z as u32).into();
        let bottom_right: TileId =
            global_to_num_space(&(self.center + vector(px_to_world, py_to_world)), z as u32).into();
        TileField::new(top_left, bottom_right)
    }

    pub fn tile_to_global_space(&self, z: f32, coordinate: &TileId) -> glm::TMat4<f32> {
        let zoom = 1.0 / 2f32.powi(coordinate.z as i32);
        let zoom = glm::scaling(&glm::vec3(zoom, zoom, 1.0));
        let pos = glm::translation(&glm::vec3(coordinate.x as f32, coordinate.y as f32, 0.0));
        self.global_to_screen(z) * zoom * pos
    }

    pub fn global_to_screen(&self, z: f32) -> glm::TMat4<f32> {
        let zoom_x = 2.0f32.powf(z) / (self.width as f32 / 2.0) * self.get_tile_size() as f32;
        let zoom_y = 2.0f32.powf(z) / (self.height as f32 / 2.0) * self.get_tile_size() as f32;
        let zoom = glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0));
        let position = glm::translation(&glm::vec3(-self.center.x, -self.center.y, 0.0));
        zoom * position
    }
}
