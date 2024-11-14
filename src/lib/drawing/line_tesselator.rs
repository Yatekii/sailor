use lyon::{
    lyon_tessellation::{FillGeometryBuilder, GeometryBuilder},
    path::Path,
};

use crate::math::{EuclidVsNalgebra, Point2, Vector2};

use super::mesh::MeshBuilder;

pub fn get_side(a: &Point2, b: &Point2, c: &Point2) -> i32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).signum() as i32
}

pub fn tesselate_line2(path: &Path, builder: &mut MeshBuilder) {
    builder.begin_geometry();
    // Fill
    let points = path.points();

    // A line always has at least 2 points.
    let first = points[0].convert();
    let second = points[1].convert();
    let mut last_line = second - first;
    let normal = Vector2::new(last_line.y, -last_line.x).normalize();
    let mut last_normal = if points.len() > 2 {
        let third = points[2].convert();
        let next_line = third - second;
        flip_vector(normal, last_line, next_line)
    } else {
        normal
    };

    let (vl, vr) = {
        let v1 = (first, last_normal);
        let v2 = (first, -last_normal);
        if get_side(&second, &first, &(first + last_normal)) == 1 {
            (v1, v2)
        } else {
            (v2, v1)
        }
    };

    let mut last_vertex_left = builder.add_vertex(vl.0.convert(), vl.1.convert()).unwrap();

    let mut last_vertex_right = builder.add_vertex(vr.0.convert(), vr.1.convert()).unwrap();

    if points.len() > 2 {
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

            let vertex_left = builder.add_vertex(vl.0.convert(), vl.1.convert()).unwrap();
            let vertex_right = builder.add_vertex(vr.0.convert(), vr.1.convert()).unwrap();

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
    }

    let last = points[points.len() - 1].convert();
    let second_last = points[points.len() - 2].convert();
    let line = last - second_last;
    let normal = Vector2::new(line.y, -line.x).normalize();

    let normal = flip_vector(normal, last_line, line);

    let (vl, vr) = {
        let v1 = (last, normal);
        let v2 = (last, -normal);
        if get_side(&last, &second_last, &(last + normal)) == 1 {
            (v1, v2)
        } else {
            (v2, v1)
        }
    };

    let vertex_left = builder.add_vertex(vl.0.convert(), vl.1.convert()).unwrap();

    let vertex_right = builder.add_vertex(vr.0.convert(), vr.1.convert()).unwrap();

    <dyn FillGeometryBuilder>::add_triangle(
        builder,
        last_vertex_left,
        last_vertex_right,
        vertex_left,
    );
    <dyn FillGeometryBuilder>::add_triangle(builder, last_vertex_right, vertex_right, vertex_left);
    builder.end_geometry();
}

fn flip_vector(normal: Vector2, last_line: Vector2, next_line: Vector2) -> Vector2 {
    let sum = last_line.normalize() + next_line.normalize();
    let dot = normal.dot(&sum);
    if (0.0..=1.0).contains(&dot) {
        normal
    } else {
        -normal
    }
}
