use lyon::path::{Event, Path};
use parry2d::{bounding_volume::Aabb, math::Vec2};

use super::Ring;

/// The area of one feature: an outer ring plus any holes or multipolygon parts.
///
/// Containment uses the even-odd rule across the rings, so holes and disjoint
/// parts are handled instead of testing one flattened, self-crossing loop.
#[derive(Debug, Clone)]
pub struct Polygon {
    rings: Vec<Ring>,
}

impl Polygon {
    /// Builds one polygon from all rings of a path (no part splitting).
    /// Rings with fewer than 3 points are dropped (they enclose no area).
    pub fn from_path(path: &Path) -> Self {
        Polygon {
            rings: split_rings(path),
        }
    }

    /// Splits a multipolygon path into its parts.
    ///
    /// MVT encodes each part as an exterior ring (one winding) followed by its
    /// holes (the opposite winding). We derive the exterior winding from the
    /// first ring with area, start a new part on each exterior ring, and attach
    /// the opposite-winding rings as holes of the current part.
    pub fn parts_from_path(path: &Path) -> Vec<Polygon> {
        let rings = split_rings(path);
        let exterior_positive = rings
            .iter()
            .map(|r| r.signed_area())
            .find(|a| *a != 0.0)
            .map(|a| a > 0.0)
            .unwrap_or(true);

        let mut parts: Vec<Polygon> = Vec::new();
        for ring in rings {
            let is_exterior = (ring.signed_area() > 0.0) == exterior_positive;
            if is_exterior || parts.is_empty() {
                parts.push(Polygon { rings: vec![ring] });
            } else {
                parts.last_mut().unwrap().rings.push(ring);
            }
        }
        parts
    }

    /// Even-odd containment across all rings.
    pub fn contains(&self, point: Vec2) -> bool {
        self.rings.iter().filter(|ring| ring.contains(point)).count() % 2 == 1
    }

    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(self.rings.iter().flat_map(|ring| ring.points().iter().copied()))
    }

    pub fn rings(&self) -> &[Ring] {
        &self.rings
    }

    /// True when no ring encloses an area.
    pub fn is_empty(&self) -> bool {
        self.rings.is_empty()
    }

    pub fn point_count(&self) -> usize {
        self.rings.iter().map(Ring::len).sum()
    }
}

/// Splits a path into rings, dropping rings with fewer than 3 points.
fn split_rings(path: &Path) -> Vec<Ring> {
    let mut rings = Vec::new();
    let mut current: Vec<Vec2> = Vec::new();
    for event in path.iter() {
        match event {
            Event::Begin { at } => current.push(Vec2::new(at.x, at.y)),
            Event::Line { to, .. } => current.push(Vec2::new(to.x, to.y)),
            Event::End { .. } => {
                if current.len() >= 3 {
                    rings.push(Ring::new(std::mem::take(&mut current)));
                } else {
                    current.clear();
                }
            }
            _ => {}
        }
    }
    if current.len() >= 3 {
        rings.push(Ring::new(current));
    }
    rings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(ox: f32, size: f32) -> Ring {
        Ring::new(vec![
            Vec2::new(ox, 0.0),
            Vec2::new(ox + size, 0.0),
            Vec2::new(ox + size, size),
            Vec2::new(ox, size),
        ])
    }

    #[test]
    fn even_odd_rejects_gap_between_multipolygon_parts() {
        // Two disjoint parts; the point in the gap sits inside the flattened bbox
        // but outside both rings and must not count as contained.
        let poly = Polygon {
            rings: vec![square(0.0, 1.0), square(10.0, 1.0)],
        };
        assert!(poly.contains(Vec2::new(0.5, 0.5)), "inside first part");
        assert!(poly.contains(Vec2::new(10.5, 0.5)), "inside second part");
        assert!(!poly.contains(Vec2::new(5.0, 0.5)), "in the gap between parts");
    }

    #[test]
    fn even_odd_rejects_hole() {
        // A big square with a smaller square hole inside it.
        let poly = Polygon {
            rings: vec![square(0.0, 10.0), square(4.0, 2.0)],
        };
        assert!(poly.contains(Vec2::new(1.0, 1.0)), "solid part");
        assert!(!poly.contains(Vec2::new(4.5, 0.5)), "inside the hole");
    }

    #[test]
    fn parts_split_by_winding() {
        // Two exteriors (CCW, positive area) each followed by a CW hole.
        let mut b = Path::builder();
        // Part 1 exterior (CCW).
        b.begin((0.0, 0.0).into());
        b.line_to((10.0, 0.0).into());
        b.line_to((10.0, 10.0).into());
        b.line_to((0.0, 10.0).into());
        b.close();
        // Part 1 hole (CW).
        b.begin((3.0, 3.0).into());
        b.line_to((3.0, 6.0).into());
        b.line_to((6.0, 6.0).into());
        b.line_to((6.0, 3.0).into());
        b.close();
        // Part 2 exterior (CCW), far away.
        b.begin((100.0, 0.0).into());
        b.line_to((110.0, 0.0).into());
        b.line_to((110.0, 10.0).into());
        b.line_to((100.0, 10.0).into());
        b.close();
        let parts = Polygon::parts_from_path(&b.build());

        assert_eq!(parts.len(), 2, "two exteriors -> two parts");
        assert_eq!(parts[0].rings().len(), 2, "part 1 keeps its hole");
        assert_eq!(parts[1].rings().len(), 1, "part 2 has no hole");
        // The hole in part 1 is respected by containment.
        assert!(parts[0].contains(Vec2::new(1.0, 1.0)));
        assert!(!parts[0].contains(Vec2::new(4.5, 4.5)), "inside the hole");
        assert!(parts[1].contains(Vec2::new(105.0, 5.0)));
    }

    #[test]
    fn drops_degenerate_rings() {
        let mut builder = Path::builder();
        // A valid triangle ...
        builder.begin((0.0, 0.0).into());
        builder.line_to((2.0, 0.0).into());
        builder.line_to((1.0, 2.0).into());
        builder.close();
        // ... and a two-point degenerate ring.
        builder.begin((5.0, 5.0).into());
        builder.line_to((6.0, 6.0).into());
        builder.close();
        let poly = Polygon::from_path(&builder.build());
        assert_eq!(poly.rings().len(), 1);
    }
}
