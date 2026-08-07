use super::*;
use nalgebra_glm as glm;

#[derive(Debug, Clone)]
pub struct Camera {
    pub center: Point,
    pub width: f32,
    pub height: f32,
    tile_size: f32,
}

impl Camera {
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

    pub fn tile_to_screen(&self, z: f32, coordinate: &TileId) -> Transform<TileLocal, Gpu> {
        let scale = 1.0 / 2f32.powi(coordinate.z as i32);
        // Offset the tile from the view center in world space (small numbers)
        // BEFORE applying the large 2^z zoom. Baking -center into the zoomed
        // matrix instead (world_to_gpu * pos) makes the translation column
        // a difference of two large products that cancels in f32 and jitters
        // the tiles by ~1px as you zoom at high z.
        let rel_x = coordinate.x as f32 * scale - self.center.x;
        let rel_y = coordinate.y as f32 * scale - self.center.y;
        let zoom_x = 2.0f32.powf(z) / (self.width / 2.0) * self.tile_size();
        let zoom_y = 2.0f32.powf(z) / (self.height / 2.0) * self.tile_size();
        Transform::from_mat(
            glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0))
                * glm::translation(&glm::vec3(rel_x, rel_y, 0.0))
                * glm::scaling(&glm::vec3(scale, scale, 1.0)),
        )
    }

    /// World -> pixels. Uniform similarity: scale `2^z · tile_size`, translate `-center`.
    pub fn world_to_screen(&self, z: f32) -> Transform<World, Pixel> {
        let s = 2.0f32.powf(z) * self.tile_size();
        Transform::from_mat(
            glm::scaling(&glm::vec3(s, s, 1.0))
                * glm::translation(&glm::vec3(-self.center.x, -self.center.y, 0.0)),
        )
    }

    /// Pixels -> NDC. Pure viewport normalization; the lone aspect-ratio step.
    pub fn screen_to_gpu(&self) -> Transform<Pixel, Gpu> {
        Transform::from_mat(glm::scaling(&glm::vec3(
            1.0 / (self.width / 2.0),
            1.0 / (self.height / 2.0),
            1.0,
        )))
    }

    pub fn world_to_gpu(&self, z: f32) -> Transform<World, Gpu> {
        self.world_to_screen(z).then(self.screen_to_gpu())
    }

    /// Transforms coordinates from pixel space to world space.
    pub fn pixel_to_world(&self, z: f32) -> Transform<Pixel, World> {
        let screen_to_global = glm::inverse(self.world_to_gpu(z).matrix());
        let translate_screen = glm::translation(&glm::vec3(-1.0, -1.0, 0.0));
        let scale_to_screen = glm::scaling(&glm::vec3(
            1.0 / (self.width / 2.0),
            1.0 / (self.height / 2.0),
            1.0,
        ));
        Transform::from_mat(screen_to_global * translate_screen * scale_to_screen)
    }

    pub fn global_to_tile_space(&self, z: f32, coordinate: &TileId) -> Transform<Gpu, TileLocal> {
        self.tile_to_screen(z, coordinate).inverse()
    }

    /// Pan the view by a pixel-space drag from `from` to `to` at zoom `z`.
    pub fn pan(&mut self, from: Coord<Pixel>, to: Coord<Pixel>, z: f32) {
        let p2w = self.pixel_to_world(z);
        let delta = p2w.apply(to).coords() - p2w.apply(from).coords();
        self.center -= vector(delta.x, delta.y);
    }

    /// Recenter so the world point under `cursor` stays fixed as zoom goes `from` -> `to`.
    pub fn zoom_to_cursor(&mut self, cursor: Coord<Pixel>, from: f32, to: f32) {
        let before = self.pixel_to_world(from).apply(cursor);
        let after = self.pixel_to_world(to).apply(cursor);
        self.center += vector(before.x() - after.x(), before.y() - after.y());
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
        let screen = Camera::new(center, 2400.0, 1400.0, 384.0, 2.0);

        let m = screen.tile_to_screen(z, &tile);
        // Transform the tile-center vertex.
        let ndc = m.apply(Coord::<TileLocal>::new(0.5, 0.5));

        // f64 reference of the same math.
        let zoom_x = 2.0f64.powf(z as f64) / (2400.0 / 2.0) * (384.0 * 2.0);
        let ref_x = zoom_x * ((tile.x as f64 + 0.5) * scale - center.x as f64);
        assert!(
            (ndc.x() as f64 - ref_x).abs() < 1e-5,
            "ndc.x={} ref={} err={}",
            ndc.x(),
            ref_x,
            (ndc.x() as f64 - ref_x).abs()
        );
    }

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    // world_to_screen is a uniform similarity: same scale on x and y (in pixels).
    #[test]
    fn world_to_screen_is_uniform() {
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0);
        let t = s.world_to_screen(3.0);
        let o = t.apply(Coord::<World>::new(0.3, 0.7)); // the center -> origin
        let dx = t.apply(Coord::<World>::new(0.4, 0.7));
        let dy = t.apply(Coord::<World>::new(0.3, 0.8));
        approx(dx.x() - o.x(), dy.y() - o.y());
        approx(o.x(), 0.0);
        approx(o.y(), 0.0);
    }

    // The split reproduces the old combined world->gpu matrix.
    #[test]
    fn world_to_gpu_equals_split() {
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0);
        let combined = s.world_to_gpu(3.0);
        let split = s.world_to_screen(3.0).then(s.screen_to_gpu());
        let p = Coord::<World>::new(0.55, 0.42);
        approx(combined.apply(p).x(), split.apply(p).x());
        approx(combined.apply(p).y(), split.apply(p).y());
    }

    // pixel -> world -> gpu -> pixel roundtrips.
    #[test]
    fn pixel_world_roundtrip() {
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0);
        let world = s.pixel_to_world(5.0).apply(Coord::<Pixel>::new(410.0, 295.0));
        let gpu = s.world_to_gpu(5.0).apply(world);
        let px = (gpu.x() + 1.0) * s.width / 2.0;
        let py = (gpu.y() + 1.0) * s.height / 2.0;
        approx(px, 410.0);
        approx(py, 295.0);
    }
}
