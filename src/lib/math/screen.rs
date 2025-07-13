use super::*;
use nalgebra_glm as glm;

#[derive(Debug, Clone)]
pub struct Screen {
    pub center: Point,
    pub width: f32,
    pub height: f32,
    tile_size: f32,
}

impl Screen {
    pub fn new(center: Point, width: f32, height: f32, tile_size: f32, hidpi_factor: f32) -> Self {
        Self {
            center,
            width,
            height,
            tile_size: tile_size * hidpi_factor,
        }
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }

    pub fn get_tile_boundaries_for_zoom_level(&self, z: f32, scale: u32) -> TileField {
        let z = z.min(14.0);
        let px_to_world = self.width / self.tile_size() / 2.0 / 2f32.powi(z as i32) / scale as f32;
        let py_to_world = self.height / self.tile_size() / 2.0 / 2f32.powi(z as i32) / scale as f32;

        let top_left: TileId =
            world_to_tile_space(&(self.center - vector(px_to_world, py_to_world)), z as u32).into();
        let bottom_right: TileId =
            world_to_tile_space(&(self.center + vector(px_to_world, py_to_world)), z as u32).into();
        TileField::new(top_left, bottom_right + TileId::new(z as u32, 1, 0))
    }

    pub fn tile_to_screen(&self, z: f32, coordinate: &TileId) -> glm::TMat4<f32> {
        let zoom = 1.0 / 2f32.powi(coordinate.z as i32);
        let zoom = glm::scaling(&glm::vec3(zoom, zoom, 1.0));
        let pos = glm::translation(&glm::vec3(coordinate.x as f32, coordinate.y as f32, 0.0));
        self.global_to_screen(z) * zoom * pos
    }

    pub fn global_to_screen(&self, z: f32) -> glm::TMat4<f32> {
        let zoom_x = 2.0f32.powf(z) / (self.width / 2.0) * self.tile_size();
        let zoom_y = 2.0f32.powf(z) / (self.height / 2.0) * self.tile_size();
        let zoom = glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0));
        let position = glm::translation(&glm::vec3(-self.center.x, -self.center.y, 0.0));
        zoom * position
    }

    pub fn screen_to_gpu(&self) -> glm::TMat4<f32> {
        let scale = glm::vec3(1.0, 1.0, 1.0).component_div(&glm::vec3(
            self.width / 2.0,
            self.height / 2.0,
            1.0,
        ));
        glm::scaling(&scale)
    }

    /// Transforms coordinates from screen space to world space.
    ///
    /// This means the ranges get transformed as follows:
    /// - [0, width] => [0, 1]
    /// - [0, height] => [0, 1]
    ///
    /// First we scale to the world space and then we also translate according to where the screen rect is currently.
    pub fn pixel_to_world(&self, z: f32) -> glm::TMat4<f32> {
        let matrix = self.global_to_screen(z);
        let screen_to_global = nalgebra_glm::inverse(&matrix);

        let translate_screen = glm::translation(&glm::vec3(-1.0, -1.0, 0.0));
        let scale_to_screen = glm::scaling(&glm::vec3(
            1.0 / (self.width / 2.0),
            1.0 / (self.height / 2.0),
            1.0,
        ));
        let pixel_to_screen = translate_screen * scale_to_screen;

        screen_to_global * pixel_to_screen
    }

    pub fn global_to_tile_space(&self, z: f32, coordinate: &TileId) -> glm::TMat4<f32> {
        self.tile_to_screen(z, coordinate).try_inverse().unwrap()
    }

    // pub fn screen_to_global(&self, z: f32) -> glm::TMat4<f32> {
    //     // self.global_to_screen(z).try_inverse().unwrap()
    //     let zoom_x = 2.0f32.powf(z) / (self.width / 2.0) * self.tile_size() * 2.0;
    //     let zoom_y = 2.0f32.powf(z) / (self.height / 2.0) * self.tile_size() * 2.0;
    //     let zoom = glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0));
    //     (zoom).try_inverse().unwrap()

    //     glm::
    // }

    // screen (px distorted) -> screen (square normalized coordinates) -> world (square coordinates) -> mercator lat lon
}
