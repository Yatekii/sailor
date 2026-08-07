use parry2d::{bounding_volume::Aabb, math::Vec2, utils::point_in_poly2d};

/// A single closed loop of points (a polygon outline or one of its holes / parts).
#[derive(Debug, Clone)]
pub struct Ring(Vec<Vec2>);

impl Ring {
    pub fn new(points: Vec<Vec2>) -> Self {
        Ring(points)
    }

    /// Whether the point lies within this ring, treating it as a closed loop.
    pub fn contains(&self, point: Vec2) -> bool {
        point_in_poly2d(point, &self.0)
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(self.0.iter().copied())
    }

    /// Signed area (shoelace formula). The sign encodes winding order; MVT uses
    /// it to distinguish exterior rings from holes.
    pub fn signed_area(&self) -> f32 {
        let n = self.0.len();
        if n < 3 {
            return 0.0;
        }
        let mut area = 0.0;
        for i in 0..n {
            let a = self.0[i];
            let b = self.0[(i + 1) % n];
            area += a.x * b.y - b.x * a.y;
        }
        area / 2.0
    }

    pub fn points(&self) -> &[Vec2] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(ox: f32) -> Ring {
        Ring::new(vec![
            Vec2::new(ox, 0.0),
            Vec2::new(ox + 1.0, 0.0),
            Vec2::new(ox + 1.0, 1.0),
            Vec2::new(ox, 1.0),
        ])
    }

    #[test]
    fn contains_inside_and_outside() {
        let r = square(0.0);
        assert!(r.contains(Vec2::new(0.5, 0.5)));
        assert!(!r.contains(Vec2::new(2.0, 0.5)));
    }
}
