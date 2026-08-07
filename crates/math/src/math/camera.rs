use super::*;
use nalgebra_glm as glm;

#[derive(Debug, Clone)]
pub struct Camera {
    pub center: PointF64,
    pub width: f32,
    pub height: f32,
    pub zoom: f32,
    tile_size: f32,
}

impl Camera {
    pub fn new(
        center: Point,
        width: f32,
        height: f32,
        tile_size: f32,
        hidpi_factor: f32,
        zoom: f32,
    ) -> Self {
        Self {
            center: PointF64::new(center.x as f64, center.y as f64),
            width,
            height,
            zoom,
            tile_size: tile_size * hidpi_factor,
        }
    }

    pub fn tile_size(&self) -> f32 {
        self.tile_size
    }

    /// Tile field covering the view at zoom level `z` (a level query, so it keeps
    /// an explicit `z`: callers ask for `zoom` and `zoom - 1` to manage the pyramid).
    pub fn get_tile_boundaries_for_zoom_level(&self, z: f32, scale: u32) -> TileField {
        let z = z.min(14.0);
        // Use the same fractional zoom as `global_to_screen` so the visible extent
        // matches what is actually rendered; using the integer floor here would
        // over-estimate the extent (up to 2x) and pull in far more than 3x3 tiles.
        let px_to_world = self.width / self.tile_size() / 2.0 / 2f32.powf(z) / scale as f32;
        let py_to_world = self.height / self.tile_size() / 2.0 / 2f32.powf(z) / scale as f32;

        // Tile selection only needs integer tile ids, so f32 is plenty here.
        let center = point(self.center.x as f32, self.center.y as f32);
        let top_left: TileId =
            world_to_tile_space(&(center - vector(px_to_world, py_to_world)), z as u32).into();
        let bottom_right: TileId =
            world_to_tile_space(&(center + vector(px_to_world, py_to_world)), z as u32).into();
        TileField::new(top_left, bottom_right + TileId::new(z as u32, 1, 0))
    }

    pub fn tile_to_screen(&self, coordinate: &TileId) -> Transform<TileLocal, Gpu> {
        let scale = 1.0 / 2f32.powi(coordinate.z as i32);
        // Offset the tile from the view center in world space (small numbers)
        // BEFORE applying the large 2^z zoom. Baking -center into the zoomed
        // matrix instead (world_to_gpu * pos) makes the translation column
        // a difference of two large products that cancels in f32 and jitters
        // the tiles by ~1px as you zoom at high z.
        // Subtract in f64 so the small result keeps its precision; casting the
        // tiny `rel` to f32 afterwards is exact enough. An f32 subtraction here
        // cancels catastrophically (both operands ~0.5) and jitters.
        let rel_x = (coordinate.x as f64 * scale as f64 - self.center.x) as f32;
        let rel_y = (coordinate.y as f64 * scale as f64 - self.center.y) as f32;
        let zoom_x = 2.0f32.powf(self.zoom) / (self.width / 2.0) * self.tile_size();
        let zoom_y = 2.0f32.powf(self.zoom) / (self.height / 2.0) * self.tile_size();
        Transform::from_mat(
            glm::scaling(&glm::vec3(zoom_x, zoom_y, 1.0))
                * glm::translation(&glm::vec3(rel_x, rel_y, 0.0))
                * glm::scaling(&glm::vec3(scale, scale, 1.0)),
        )
    }

    /// World -> pixels. Uniform similarity: scale `2^z · tile_size`, translate `-center`.
    pub fn world_to_screen(&self) -> Transform<World, Pixel> {
        let s = 2.0f32.powf(self.zoom) * self.tile_size();
        Transform::from_mat(
            glm::scaling(&glm::vec3(s, s, 1.0))
                * glm::translation(&glm::vec3(-self.center.x as f32, -self.center.y as f32, 0.0)),
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

    pub fn world_to_gpu(&self) -> Transform<World, Gpu> {
        self.world_to_screen().then(self.screen_to_gpu())
    }

    /// Transforms coordinates from pixel space to world space.
    pub fn pixel_to_world(&self) -> Transform<Pixel, World> {
        let screen_to_global = glm::inverse(self.world_to_gpu().matrix());
        let translate_screen = glm::translation(&glm::vec3(-1.0, -1.0, 0.0));
        let scale_to_screen = glm::scaling(&glm::vec3(
            1.0 / (self.width / 2.0),
            1.0 / (self.height / 2.0),
            1.0,
        ));
        Transform::from_mat(screen_to_global * translate_screen * scale_to_screen)
    }

    pub fn global_to_tile_space(&self, coordinate: &TileId) -> Transform<Gpu, TileLocal> {
        self.tile_to_screen(coordinate).inverse()
    }

    /// Pan the view by a pixel-space drag from `from` to `to` at the current zoom.
    ///
    /// The world delta is `(to - from) / s`; the `center` term of `pixel_to_world`
    /// cancels analytically, so we compute it directly in f64 instead of
    /// subtracting two absolute world positions (which cancel in f32).
    pub fn pan(&mut self, from: Coord<Pixel>, to: Coord<Pixel>) {
        let s = 2f64.powf(self.zoom as f64) * self.tile_size() as f64;
        self.center.x -= (to.x() as f64 - from.x() as f64) / s;
        self.center.y -= (to.y() as f64 - from.y() as f64) / s;
    }

    /// Recenter so the world point under `cursor` stays fixed as zoom goes from the
    /// current zoom to `to`, then adopt `to`.
    ///
    /// `before - after` equals `(cursor - screen_center) * (1/s_from - 1/s_to)`;
    /// the shared `center` cancels analytically, so we compute the tiny delta
    /// directly in f64 and accumulate it into the f64 `center`. Doing it via two
    /// absolute world positions cancels in f32 and, together with an f32 center,
    /// snapped the view as you zoomed.
    pub fn zoom_to_cursor(&mut self, cursor: Coord<Pixel>, to: f32) {
        let s_from = 2f64.powf(self.zoom as f64) * self.tile_size() as f64;
        let s_to = 2f64.powf(to as f64) * self.tile_size() as f64;
        let inv = 1.0 / s_from - 1.0 / s_to;
        self.center.x += (cursor.x() as f64 - self.width as f64 / 2.0) * inv;
        self.center.y += (cursor.y() as f64 - self.height as f64 / 2.0) * inv;
        self.zoom = to;
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
        let camera = Camera::new(center, 2400.0, 1400.0, 384.0, 2.0, z);

        let m = camera.tile_to_screen(&tile);
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

    // Zooming to the cursor in many tiny steps must land on the same center as
    // one big step (the deltas telescope). With an f32 center each step rounds
    // to a ~pixel grid at high zoom and the two diverge — that was the jitter.
    #[test]
    fn zoom_to_cursor_accumulates_precisely() {
        let cursor = Coord::<Pixel>::new(1700.0, 300.0); // well off-centre
        let make = || Camera::new(point(0.5187345, 0.5093721), 2400.0, 1400.0, 384.0, 2.0, 14.0);

        // Many small scroll steps from z14 up to ~z18.
        let mut stepwise = make();
        for _ in 0..2000 {
            let to = stepwise.zoom + 0.002;
            stepwise.zoom_to_cursor(cursor, to);
        }
        let end_zoom = stepwise.zoom;

        // One big step to the exact same end zoom.
        let mut oneshot = make();
        oneshot.zoom_to_cursor(cursor, end_zoom);

        // Difference in world space, expressed in pixels at the final zoom.
        let s = 2f64.powf(end_zoom as f64) * oneshot.tile_size() as f64;
        let err_px = ((oneshot.center.x - stepwise.center.x).powi(2)
            + (oneshot.center.y - stepwise.center.y).powi(2))
        .sqrt()
            * s;
        assert!(err_px < 0.5, "cursor-anchored zoom drifted {err_px} px");
    }

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-4, "{a} != {b}");
    }

    // world_to_screen is a uniform similarity: same scale on x and y (in pixels).
    #[test]
    fn world_to_screen_is_uniform() {
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0, 3.0);
        let t = s.world_to_screen();
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
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0, 3.0);
        let combined = s.world_to_gpu();
        let split = s.world_to_screen().then(s.screen_to_gpu());
        let p = Coord::<World>::new(0.55, 0.42);
        approx(combined.apply(p).x(), split.apply(p).x());
        approx(combined.apply(p).y(), split.apply(p).y());
    }

    // pixel -> world -> gpu -> pixel roundtrips.
    #[test]
    fn pixel_world_roundtrip() {
        let s = Camera::new(point(0.3, 0.7), 800.0, 600.0, 256.0, 1.0, 5.0);
        let world = s.pixel_to_world().apply(Coord::<Pixel>::new(410.0, 295.0));
        let gpu = s.world_to_gpu().apply(world);
        let px = (gpu.x() + 1.0) * s.width / 2.0;
        let py = (gpu.y() + 1.0) * s.height / 2.0;
        approx(px, 410.0);
        approx(py, 295.0);
    }
}
