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

pub fn tesselate_line2(path: &Path, builder: &mut MeshBuilder) {
    // Helper matrixes to be reused.
    let rot_90: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(PI / 2.0));
    let rot_45: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(PI / 4.0));
    let mrot_90: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(-PI / 2.0));

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

    let mut last_vertex_left = builder.add_vertex(first, normal).unwrap();
    let mut last_vertex_right = builder.add_vertex(first, rot_90 * normal).unwrap();
    let mut last_normal = normal;

    for i in 0..points.len() - 2 {
        let previous = points[i].convert();
        let current = points[i + 1].convert();
        let next = points[i + 2].convert();
        let current_line = current - previous;
        let next_line = current - next;

        let normal = current_line.normalize() + next_line.normalize();
        let local_normal = Vector2::new(last_line.y, -last_line.x);
        let dot = local_normal.dot(&last_normal);
        let local_normal = if (0.0..=1.0).contains(&dot) {
            local_normal
        } else {
            -local_normal
        }
        .normalize();

        let dot = local_normal.dot(&normal);
        let normal = if dot == 0.0 { local_normal } else { normal }.normalize();

        let factor = (1.0 / normal.dot(&local_normal).abs()).min(3.0);

        let (vl, vr) = {
            let v1 = (current, normal * factor);
            let v2 = (current, -normal * factor);
            if get_side(&current, &previous, &(current + normal)) == 1 {
                (v1, v2)
            } else {
                (v2, v1)
            }
        };

        let vertex_left = builder.add_vertex(vl.0, vl.1).unwrap();
        let vertex_right = builder.add_vertex(vr.0, vr.1).unwrap();

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
        last_normal = normal;
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

    let vertex_left = builder.add_vertex(last, normal).unwrap();
    let vertex_right = builder.add_vertex(last, mrot_90 * normal).unwrap();

    <dyn FillGeometryBuilder>::add_triangle(
        builder,
        last_vertex_left,
        last_vertex_right,
        vertex_left,
    );
    <dyn FillGeometryBuilder>::add_triangle(builder, last_vertex_right, vertex_right, vertex_left);
    builder.end_geometry();
}
