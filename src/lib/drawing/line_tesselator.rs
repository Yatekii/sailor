use std::f32::consts::PI;

use lyon::{
    lyon_tessellation::{FillGeometryBuilder, GeometryBuilder},
    path::Path,
};
use nalgebra::Vector1;

use crate::math::{EuclidVsNalgebra, Point2, Rotation2};

use super::mesh::MeshBuilder;

pub fn get_side(a: &Point2, b: &Point2, c: &Point2) -> i32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).signum() as i32
}

pub fn tesselate_line2(path: &Path, builder: &mut MeshBuilder, extent: f32) {
    // Helper matrixes to be reused.
    let rot_90: Rotation2 = Rotation2::from_scaled_axis(Vector1::new(PI / 2.0));
    let factor = (1.0 / (PI / 2.0).sin()).abs();

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
    // The normal at the linecap is 90 degrees.

    //        |
    //        | Normal 1
    //  first |_______ second
    //        |
    //        |   Line (two triangles)
    //        |_______
    //        |
    //        |  Normal 2
    //        |

    // A line always has at least 2 points. If not, we have a bug.
    // TODO: Harden this against potential faulty tiles.
    let first = points[0].convert();
    let second = points[1].convert();
    let mut last_line = second - first;
    let normal = (rot_90 * last_line).normalize();

    let mut last_vertex_left = builder.add_vertex(first, normal * extent * factor);
    let mut last_vertex_right = builder.add_vertex(first, -normal * extent * factor);

    for i in 0..points.len() - 2 {
        let current = points[i + 1].convert();
        let next = points[i + 2].convert();
        let next_line = next - current;

        // Miter normal from the two segment PERPENDICULARS, not the segment
        // directions. On a near-straight joint the directions are nearly
        // opposite, so their sum cancels to ~0 and normalizing it yields a
        // numerically unstable direction pointing anywhere — the random spikes.
        // The perpendiculars instead add up, staying stable and perpendicular.
        // Both are left-normals, so their sum is a consistent left-pointing miter.
        let n_in = (rot_90 * last_line).normalize();
        let n_out = (rot_90 * next_line).normalize();
        let normal = (n_in + n_out).normalize();

        // ponytail: miter limit. At sharp/hairpin joins 1/|cos θ| → ∞ and the
        // corner shoots a spike to infinity; clamp it. 4.0 ≈ SVG's default miter
        // limit (~29° min angle). A degenerate NaN factor also collapses to the
        // limit via min. Upgrade to bevel/round joins if clamped corners show.
        let factor = (1.0 / normal.dot(&n_in).abs()).min(4.0);

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
    // We do the same as for the first two points of the line string but only rotate 90 degrees.
    // Which makes a 90 degree angle between this normal and the first one, which makes sense if you think
    // about it for a second :)

    // And the second normal is again 90 degrees further, but this time clockwise.

    // The line string has at least two points (same assumption as before).
    let last = points[points.len() - 1].convert();
    let second_last = points[points.len() - 2].convert();
    let line = last - second_last;
    let normal = (rot_90 * line).normalize();

    let vertex_left = builder.add_vertex(last, normal * extent * factor);
    let vertex_right = builder.add_vertex(last, -normal * extent * factor);

    <dyn FillGeometryBuilder>::add_triangle(
        builder,
        last_vertex_left,
        last_vertex_right,
        vertex_left,
    );
    <dyn FillGeometryBuilder>::add_triangle(builder, last_vertex_right, vertex_right, vertex_left);

    builder.end_geometry();
}

#[cfg(test)]
mod tests {
    use lyon::{math::Point, path::Path, tessellation::VertexBuffers};

    use crate::{
        drawing::{
            mesh::MeshBuilder,
            vertex::{LayerVertexCtor, VertexType},
        },
        math::TileId,
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
                object_id: 0,
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
                object_id: 0,
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
                object_id: 0,
            },
        );

        tesselate_line2(&path, &mut builder, extent);

        insta::assert_debug_snapshot!(builder.buffers);
    }

    #[test]
    fn hairpin_join_stays_within_miter_limit() {
        // A near-180° reversal drives the miter factor toward infinity. Without the
        // clamp the stored i16 normals saturate to 32767 (the visible spike); with
        // it they stay within extent * miter_limit (4096 * 4 = 16384).
        let extent = 4096.0;
        let mut builder = Path::builder();
        builder.begin(Point::new(0.0, 0.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.line_to(Point::new(0.0, 0.01));
        builder.end(false);
        let path = builder.build();

        let mut buffers = VertexBuffers::new();
        let mut mesh = MeshBuilder::new(
            &mut buffers,
            LayerVertexCtor {
                tile_id: TileId::new(0, 0, 0),
                feature_id: 0,
                extent,
                vertex_type: VertexType::Line,
                object_id: 0,
            },
        );
        tesselate_line2(&path, &mut mesh, extent);

        for v in &mesh.buffers.vertices {
            let normal = v.normal; // packed struct: copy out before use
            let limit = extent * 4.0 + 1.0;
            assert!(
                (normal[0].unsigned_abs() as f32) <= limit
                    && (normal[1].unsigned_abs() as f32) <= limit,
                "normal {normal:?} exceeds miter limit",
            );
        }
    }

    #[test]
    fn near_straight_join_stays_perpendicular() {
        // A tiny kink in an almost-straight horizontal line. The old direction-sum
        // miter normalized a ~0 vector here and pointed in a random direction (the
        // spikes on straight roads). The perpendicular-sum miter stays perpendicular
        // (~(0, ±extent)), so the along-line component must stay near zero.
        let extent = 4096.0;
        let mut builder = Path::builder();
        builder.begin(Point::new(0.0, 0.0));
        builder.line_to(Point::new(1.0, 0.0));
        builder.line_to(Point::new(2.0, 0.001));
        builder.end(false);
        let path = builder.build();

        let mut buffers = VertexBuffers::new();
        let mut mesh = MeshBuilder::new(
            &mut buffers,
            LayerVertexCtor {
                tile_id: TileId::new(0, 0, 0),
                feature_id: 0,
                extent,
                vertex_type: VertexType::Line,
                object_id: 0,
            },
        );
        tesselate_line2(&path, &mut mesh, extent);

        for v in &mesh.buffers.vertices {
            let normal = v.normal; // packed struct: copy out before use
            assert!(
                normal[0].unsigned_abs() <= normal[1].unsigned_abs(),
                "normal {normal:?} points along the line — unstable miter",
            );
        }
    }
}
