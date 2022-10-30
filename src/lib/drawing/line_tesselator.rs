use lyon::{
    lyon_tessellation::{FillGeometryBuilder, GeometryBuilder},
    math::*,
    path::Path,
};

use crate::*;

pub fn get_side(a: &Point, b: &Point, c: &Point) -> i32 {
    ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)).signum() as i32
}

pub fn tesselate_line2(path: &Path, builder: &mut MeshBuilder, z: u32) {
    builder.begin_geometry();
    // Fill
    let points = path.points();

    let width_factor = 2f32.powi(z as i32 - 14);

    let first = points[0];
    let second = points[1];
    let mut last_line = second.to_vector() - first.to_vector();
    let normal = vector(last_line.y, -last_line.x);
    let mut last_normal = if points.len() > 2 {
        let third = points[2];
        let next_line = third.to_vector() - second.to_vector();
        let dot = normal.dot(last_line.normalize() + next_line.normalize());
        if (0.0..=1.0).contains(&dot) {
            normal
        } else {
            -normal
        }
    } else {
        normal
    }
    .normalize()
        * width_factor;

    let (vl, vr) = {
        let v1 = (first, last_normal);
        let v2 = (first, -last_normal);
        if get_side(&second, &first, &(first + last_normal)) == 1 {
            (v1, v2)
        } else {
            (v2, v1)
        }
    };

    let mut last_vertex_left = builder.add_vertex(vl.0, vl.1).unwrap();

    let mut last_vertex_right = builder.add_vertex(vr.0, vr.1).unwrap();

    if points.len() > 2 {
        for i in 0..points.len() - 2 {
            let previous = points[i];
            let current = points[i + 1];
            let next = points[i + 2];
            let current_line = current.to_vector() - previous.to_vector();
            let next_line = current.to_vector() - next.to_vector();

            let normal = current_line.normalize() + next_line.normalize();
            let local_normal = vector(last_line.y, -last_line.x);
            let dot = local_normal.dot(last_normal);
            let local_normal = if (0.0..=1.0).contains(&dot) {
                local_normal
            } else {
                -local_normal
            }
            .normalize();

            let dot = local_normal.dot(normal);
            let normal = if dot == 0.0 { local_normal } else { normal }.normalize() * width_factor;

            let factor = (1.0 / normal.dot(local_normal).abs()).min(3.0);

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
    }

    let last = points[points.len() - 1];
    let second_last = points[points.len() - 2];
    let line = last.to_vector() - second_last.to_vector();
    let normal: Vector = vector(line.y, -line.x).normalize() * width_factor;

    let dot = normal.dot(last_line.normalize() + line.normalize());

    let normal = if (0.0..=1.0).contains(&dot) {
        normal
    } else {
        -normal
    };

    let (vl, vr) = {
        let v1 = (last, normal);
        let v2 = (last, -normal);
        if get_side(&last, &second_last, &(last + normal * width_factor)) == 1 {
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
    <dyn FillGeometryBuilder>::add_triangle(builder, last_vertex_right, vertex_right, vertex_left);
    builder.end_geometry();
}
