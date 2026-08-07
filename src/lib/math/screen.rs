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
        // Use the same fractional zoom as `global_to_screen` so the visible extent
        // matches what is actually rendered; using the integer floor here would
        // over-estimate the extent (up to 2x) and pull in far more than 3x3 tiles.
        let px_to_world = self.width / self.tile_size() / 2.0 / 2f32.powf(z) / scale as f32;
        let py_to_world = self.height / self.tile_size() / 2.0 / 2f32.powf(z) / scale as f32;

        let top_left: TileId =
            world_to_tile_space(&(self.center - vector(px_to_world, py_to_world)), z as u32).into();
        let bottom_right: TileId =
            world_to_tile_space(&(self.center + vector(px_to_world, py_to_world)), z as u32).into();
        TileField::new(top_left, bottom_right + TileId::new(z as u32, 1, 0))
    }

    pub fn tile_to_screen(&self, z: f32, coordinate: &TileId) -> glm::TMat4<f32> {
        let scale = 1.0 / 2f32.powi(coordinate.z as i32);
        // Offset the tile from the view center in world space (small numbers)
        // BEFORE applying the large 2^z zoom. Baking -center into the zoomed
        // matrix instead (global_to_screen * pos) makes the translation column
        // a difference of two large products that cancels in f32 and jitters
        // the tiles by ~1px as you zoom at high z.
        let rel_x = coordinate.x as f32 * scale - self.center.x;
        let rel_y = coordinate.y as f32 * scale - self.center.y;
        let zoom_x = 2.0f32.powf(z) / (self.width / 2.0) * self.tile_size();
        let zoom_y = 2.0f32.powf(z) / (self.height / 2.0) * self.tile_size();
        glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0))
            * glm::translation(&glm::vec3(rel_x, rel_y, 0.0))
            * glm::scaling(&glm::vec3(scale, scale, 1.0))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // Regression guard: at high zoom the tile transform must not lose precision
    // to catastrophic cancellation. Baking -center into the zoomed matrix (the
    // old `global_to_screen * pos` grouping) put ~2e-4 NDC of f32 error here,
    // ~0.1px, which jittered per-tile as you zoomed. Subtracting in world space
    // first keeps it near f64.
    #[test]
    fn tile_transform_stays_precise_at_high_zoom() {
        let z = 18.0;
        let tz = 14u32;
        let scale = 1.0 / 2f64.powi(tz as i32);
        // View centered mid-world so both operands of the subtraction are ~0.5.
        let center = point(0.5187345, 0.5093721);
        let tile = TileId::new(tz, (0.5187 / scale) as u32, (0.5093 / scale) as u32);
        let screen = Screen::new(center, 2400.0, 1400.0, 384.0, 2.0);

        let m = screen.tile_to_screen(z, &tile);
        // Transform the tile-center vertex.
        let ndc = m * glm::vec4(0.5, 0.5, 0.0, 1.0);

        // f64 reference of the same math.
        let zoom_x = 2.0f64.powf(z as f64) / (2400.0 / 2.0) * (384.0 * 2.0);
        let ref_x = zoom_x * ((tile.x as f64 + 0.5) * scale - center.x as f64);
        assert!(
            (ndc.x as f64 - ref_x).abs() < 1e-5,
            "ndc.x={} ref={} err={}",
            ndc.x,
            ref_x,
            (ndc.x as f64 - ref_x).abs()
        );
    }
}
