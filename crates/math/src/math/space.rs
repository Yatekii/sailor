use nalgebra_glm as glm;
use std::marker::PhantomData;

// Coordinate space markers. Uninhabited — only used at the type level.
pub enum Geo {}
pub enum TileLocal {}
pub enum World {}
pub enum Pixel {}
pub enum Gpu {}

/// A 2D point tagged with the space `S` it lives in.
pub struct Coord<S>(glm::Vec2, PhantomData<S>);

impl<S> Coord<S> {
    pub fn new(x: f32, y: f32) -> Self {
        Coord(glm::vec2(x, y), PhantomData)
    }
    pub fn coords(&self) -> glm::Vec2 {
        self.0
    }
    pub fn x(&self) -> f32 {
        self.0.x
    }
    pub fn y(&self) -> f32 {
        self.0.y
    }
}

impl<S> Clone for Coord<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> Copy for Coord<S> {}

/// An affine transform from space `A` to space `B`, backed by a 4x4 matrix.
pub struct Transform<A, B>(glm::TMat4<f32>, PhantomData<(A, B)>);

impl<A, B> Transform<A, B> {
    pub fn from_mat(m: glm::TMat4<f32>) -> Self {
        Transform(m, PhantomData)
    }
    /// The raw matrix, e.g. for GPU upload.
    pub fn matrix(&self) -> &glm::TMat4<f32> {
        &self.0
    }
    pub fn apply(&self, p: Coord<A>) -> Coord<B> {
        let v = self.0 * glm::vec4(p.0.x, p.0.y, 0.0, 1.0);
        Coord(glm::vec2(v.x, v.y), PhantomData)
    }
    pub fn inverse(&self) -> Transform<B, A> {
        Transform(glm::inverse(&self.0), PhantomData)
    }
    /// Compose: first `self` (A->B), then `next` (B->C).
    pub fn then<C>(self, next: Transform<B, C>) -> Transform<A, C> {
        Transform(next.0 * self.0, PhantomData)
    }
}

impl<A, B> Clone for Transform<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<A, B> Copy for Transform<A, B> {}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm as glm;

    enum A {}
    enum B {}

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 1e-5, "{a} != {b}");
    }

    #[test]
    fn apply_translates_and_scales() {
        // scale x2, then translate (+1, +3)
        let t: Transform<A, B> = Transform::from_mat(
            glm::translation(&glm::vec3(1.0, 3.0, 0.0)) * glm::scaling(&glm::vec3(2.0, 2.0, 1.0)),
        );
        let p = t.apply(Coord::<A>::new(4.0, 5.0));
        approx(p.x(), 9.0);
        approx(p.y(), 13.0);
    }

    #[test]
    fn inverse_roundtrips() {
        let t: Transform<A, B> = Transform::from_mat(
            glm::translation(&glm::vec3(1.0, 3.0, 0.0)) * glm::scaling(&glm::vec3(2.0, 4.0, 1.0)),
        );
        let p = Coord::<A>::new(4.0, 5.0);
        let back = t.inverse().apply(t.apply(p));
        approx(back.x(), 4.0);
        approx(back.y(), 5.0);
    }

    #[test]
    fn then_composes_in_order() {
        let ab: Transform<A, B> = Transform::from_mat(glm::scaling(&glm::vec3(2.0, 2.0, 1.0)));
        let bc: Transform<B, A> = Transform::from_mat(glm::translation(&glm::vec3(1.0, 1.0, 0.0)));
        // first ab (scale), then bc (translate)
        let p = ab.then(bc).apply(Coord::<A>::new(3.0, 3.0));
        approx(p.x(), 7.0);
        approx(p.y(), 7.0);
    }
}
