use std::f32::consts::PI;

use lyon::{
    lyon_tessellation::{FillGeometryBuilder, GeometryBuilder},
    path::Path,
};
use nalgebra::Vector1;

use crate::math::{EuclidVsNalgebra, Point2, Rotation2, Vector2};

use super::mesh::MeshBuilder;

pub fn get_side(a: &Point2, b: &Point2, c: &Point2) -> i32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).signum() as i32
}

pub fn tesselate_line2(path: &Path, builder: &mut MeshBuilder, extent: f32) {
    // Helper matrixes to be reused.
    let rot_90: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(PI / 2.0));
    let rot_45: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(PI / 4.0));
    let mrot_90: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(-PI / 2.0));
    let factor = (1.0 / (PI / 4.0).sin()).abs();

    // Start a new tesselation geometry.
    builder.begin_geometry();

    // We go over all points in the path and calculate the normal for each point.
    // Then we march along the normal for each point and draw triangles that go to a point on that normal
    // to have a line thickness.
    // Each line segment will consist of two triangles that form a trapezoid.
    // The trapezoid can be imagined as a rectangle with cut off tops and bottoms so the line string corners
    // do not have weird artifacts and join nicely.

    let points = path.points();

    // We look at the first two points to calculate the first special normal which is necessary to
    // have a nice and flat line cap.
    // The normal at the linecap is 45 degrees.

    //      \
    //       \   Normal 1
    //  first \_______ second
    //        |
    //        |   Line (two triangles)
    //        |_______
    //        /
    //       /   Normal 2
    //      /

    // A line always has at least 2 points. If not, we have a bug.
    // TODO: Harden this against potential faulty tiles.
    let first = points[0].convert();
    let second = points[1].convert();
    let mut last_line = second - first;
    let normal = (rot_90 * rot_45 * last_line).normalize();

    let mut last_vertex_left = builder.add_vertex(first, normal * extent * factor);
    let mut last_vertex_right = builder.add_vertex(first, rot_90 * normal * extent * factor);

    for i in 0..points.len() - 2 {
        let previous = points[i].convert();
        let current = points[i + 1].convert();
        let next = points[i + 2].convert();
        let current_line = previous - current;
        let next_line = next - current;

        let mut normal = (current_line.normalize() + next_line.normalize()).normalize();
        if is_ccw_angle(current_line, normal) {
            normal = -normal;
        }

        let local_normal = (rot_90 * last_line).normalize();

        let factor =
            1.0 / (normal.dot(&local_normal) / (normal.norm() * local_normal.norm())).abs();

        let vertex_left = builder.add_vertex(current, normal * extent * factor);
        let vertex_right = builder.add_vertex(current, -normal * extent * factor);

        <dyn FillGeometryBuilder>::add_triangle(
            builder,
            last_vertex_left,
            last_vertex_right,
            vertex_left,
        );
        <dyn FillGeometryBuilder>::add_triangle(
            builder,
            last_vertex_right,
            vertex_right,
            vertex_left,
        );

        last_vertex_left = vertex_left;
        last_vertex_right = vertex_right;
        last_line = next_line;
    }

    // We take the last two points that also need a special normal calculation.
    // We do the same as for the first two points of the line string but only rotate 45 degrees.
    // Which makes a 90 degree angle between this normal and the first one, which makes sense if you think
    // about it for a second :)

    // And the second normal is again 90 degrees further, but this trime clockwise.

    // The line string has at least two points (same assumption as before).
    let last = points[points.len() - 1].convert();
    let second_last = points[points.len() - 2].convert();
    let line = last - second_last;
    let normal = (rot_45 * line).normalize();

    let vertex_left = builder.add_vertex(last, normal * extent * factor);
    let vertex_right = builder.add_vertex(last, mrot_90 * normal * extent * factor);

    <dyn FillGeometryBuilder>::add_triangle(
        builder,
        last_vertex_left,
        last_vertex_right,
        vertex_left,
    );
    <dyn FillGeometryBuilder>::add_triangle(builder, last_vertex_right, vertex_right, vertex_left);

    builder.end_geometry();
}

/// Positive angle means CCW, negative means CW.
fn ccw_angle(a: Vector2, b: Vector2) -> f32 {
    let dot = a.x * b.x + a.y * b.y;
    let det = a.x * b.y - a.y * b.x;
    det.atan2(dot)
}

fn is_ccw_angle(a: Vector2, b: Vector2) -> bool {
    ccw_angle(a, b) >= 0.0
}

#[cfg(test)]
mod tests {
    use std::f32::consts::PI;

    use lyon::{math::Point, path::Path, tessellation::VertexBuffers};

    use crate::{
        drawing::{
            line_tesselator::{ccw_angle, is_ccw_angle},
            mesh::MeshBuilder,
            vertex::{LayerVertexCtor, VertexType},
        },
        math::{TileId, Vector2},
    };

    use super::tesselate_line2;

    #[test]
    fn tesselate_straight_line() {
        let extent = 4096.0;

        let mut builder = Path::builder();
        builder.begin(Point::new(0.0, 0.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.end(false);
        let path = builder.build();

        let mut buffers = VertexBuffers::new();
        let mut builder = MeshBuilder::new(
            &mut buffers,
            LayerVertexCtor {
                tile_id: TileId::new(0, 0, 0),
                feature_id: 0,
                extent,
                vertex_type: VertexType::Line,
            },
        );

        tesselate_line2(&path, &mut builder, extent);

        insta::assert_debug_snapshot!(builder.buffers);
    }

    #[test]
    fn tesselate_90deg_angle_line() {
        let extent = 4096.0;

        let mut builder = Path::builder();
        builder.begin(Point::new(0.0, 0.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.line_to(Point::new(1.0, 1.0));
        builder.end(false);
        let path = builder.build();

        let mut buffers = VertexBuffers::new();
        let mut builder = MeshBuilder::new(
            &mut buffers,
            LayerVertexCtor {
                tile_id: TileId::new(0, 0, 0),
                feature_id: 0,
                extent,
                vertex_type: VertexType::Line,
            },
        );

        tesselate_line2(&path, &mut builder, extent);

        insta::assert_debug_snapshot!(builder.buffers);
    }

    #[test]
    fn tesselate_zigzag_line() {
        let extent = 4096.0;

        let mut builder = Path::builder();
        builder.begin(Point::new(0.0, 0.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.line_to(Point::new(1.0, 1.0));
        builder.line_to(Point::new(2.0, 1.0));
        builder.end(false);
        let path = builder.build();

        let mut buffers = VertexBuffers::new();
        let mut builder = MeshBuilder::new(
            &mut buffers,
            LayerVertexCtor {
                tile_id: TileId::new(0, 0, 0),
                feature_id: 0,
                extent,
                vertex_type: VertexType::Line,
            },
        );

        tesselate_line2(&path, &mut builder, extent);

        insta::assert_debug_snapshot!(builder.buffers);
    }

    #[test]
    fn test_ccw_angle() {
        let a = Vector2::new(0.0, -1.0);
        let b = Vector2::new(1.0, 0.0);

        let angle = ccw_angle(a, b);
        let is_ccw = is_ccw_angle(a, b);

        assert_eq!(angle, PI / 2.0);
        assert!(is_ccw);
    }

    #[test]
    fn test_cw_angle() {
        let b = Vector2::new(0.0, -1.0);
        let a = Vector2::new(1.0, 0.0);

        let angle = ccw_angle(a, b);
        let is_ccw = is_ccw_angle(a, b);

        assert_eq!(angle, -PI / 2.0);
        assert!(!is_ccw);
    }
}
