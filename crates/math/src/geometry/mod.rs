mod polygon;
mod ring;

pub use polygon::Polygon;
pub use ring::Ring;

use lyon::path::{Event, Path};
use parry2d::math::Vec2;

/// The geometry of a map feature. Only polygons enclose an area and take part in
/// hit-testing; lines and points are kept for completeness (labels, size accounting).
#[derive(Debug, Clone)]
pub enum Geometry {
    Polygon(Polygon),
    Line(Vec<Vec2>),
    Point(Vec2),
}

impl Geometry {
    pub fn polygon(path: &Path) -> Self {
        Geometry::Polygon(Polygon::from_path(path))
    }

    pub fn line(path: &Path) -> Self {
        Geometry::Line(path_points(path))
    }

    pub fn point(path: &Path) -> Self {
        Geometry::Point(path_points(path).first().copied().unwrap_or(Vec2::new(0.0, 0.0)))
    }

    /// A short, stable kind name (used by tests / debug output).
    pub fn kind(&self) -> &'static str {
        match self {
            Geometry::Polygon(_) => "polygon",
            Geometry::Line(_) => "line",
            Geometry::Point(_) => "point",
        }
    }

    /// Number of stored vertices, for memory accounting.
    pub fn point_count(&self) -> usize {
        match self {
            Geometry::Polygon(p) => p.point_count(),
            Geometry::Line(points) => points.len(),
            Geometry::Point(_) => 1,
        }
    }
}

/// Collects every vertex of a path into a flat list (ignores ring boundaries).
fn path_points(path: &Path) -> Vec<Vec2> {
    let mut points = Vec::new();
    for event in path.iter() {
        match event {
            Event::Begin { at } => points.push(Vec2::new(at.x, at.y)),
            Event::Line { to, .. } => points.push(Vec2::new(to.x, to.y)),
            _ => {}
        }
    }
    points
}
