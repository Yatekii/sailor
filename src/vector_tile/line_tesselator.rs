use crate::drawing::mesh::MeshBuilder;
use crate::lyon::lyon_tessellation::GeometryBuilder;
use lyon::path::Path;
use lyon::math::*;
use lyon::tessellation::{
    FillVertex,
};

const LINE_WIDTH: f32 = 12.0;

pub fn get_side(a: &Point, b: &Point, c: &Point) -> i32 {
    num_traits::sign::signum((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) as i32
}

pub fn tesselate_line2<'a, 'l>(path: &Path, builder: &'a mut MeshBuilder<'l>) {
    GeometryBuilder::<FillVertex>::begin_geometry(builder);
    // Fill
    builder.set_current_vertex_type(false);
    let points = path.points();

    let first = points[0];
    let second = points[1];
    let mut last_line = second.to_vector() - first.to_vector();
    let normal = vector(last_line.y, -last_line.x);
    let mut last_normal = if points.len() > 2 {
        let third = points[2];
        let next_line = third.to_vector() - second.to_vector();
        let dot = normal.dot(last_line.normalize() + next_line.normalize());
        if dot <= 1.0 && dot >= 0.0 {
            normal
        } else {
            -normal
        }
    } else {
        normal
    }.normalize() * LINE_WIDTH;

    let (vl, vr) = {
        let v1 = (first + last_normal, last_normal);
        let v2 = (first - last_normal, -last_normal);
        if get_side(&second, &first, &v1.0) == 1 {
            (v1, v2)
        } else {
            (v2, v1)
        }
    };

    let mut last_vertex_left = builder.add_vertex(FillVertex {
        position: vl.0,
        normal: vl.1,
    }).unwrap();

    let mut last_vertex_right = builder.add_vertex(FillVertex {
        position: vr.0,
        normal: vr.1,
    }).unwrap();

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
            let local_normal = if dot <= 1.0 && dot >= 0.0 {
                local_normal
            } else {
                -local_normal
            }.normalize();

            let dot = local_normal.dot(normal);
            let normal = if dot == 0.0 {
                local_normal
            } else {
                normal
            }.normalize();

            let factor = 1.0 / normal.dot(local_normal) * LINE_WIDTH;
            // let factor = 1.0;

            let (vl, vr) = {
                let v1 = (current + normal * factor, normal * factor);
                let v2 = (current - normal * factor, -normal * factor);
                if get_side(&current, &previous, &v1.0) == 1 {
                    (v1, v2)
                } else {
                    (v2, v1)
                }
            };

            let vertex_left = builder.add_vertex(FillVertex {
                position: vl.0,
                normal: vl.1,
            }).unwrap();

            let vertex_right = builder.add_vertex(FillVertex {
                position: vr.0,
                normal: vr.1,
            }).unwrap();

            GeometryBuilder::<FillVertex>::add_triangle(
                builder,
                last_vertex_left,
                last_vertex_right,
                vertex_left
            );
            GeometryBuilder::<FillVertex>::add_triangle(
                builder,
                last_vertex_right,
                vertex_right,
                vertex_left
            );

            last_vertex_left = vertex_left;
            last_vertex_right = vertex_right;
            last_normal = normal;
            last_line = next_line;
        }
    }

    let line = points[points.len() - 1].to_vector() - points[points.len() - 2].to_vector();
    let normal: Vector = vector(line.y, -line.x).normalize() * LINE_WIDTH;

    let dot = normal.dot(last_line.normalize() + line.normalize());

    let normal = if dot <= 1.0 && dot >= 0.0 {
        normal
    } else {
        -normal
    };

    let (vl, vr) = {
        let v1 = (points[points.len() - 1] + normal, normal);
        let v2 = (points[points.len() - 1] - normal, -normal);
        if get_side(&points[points.len() - 1], &points[points.len() - 2], &v1.0) == 1 {
            (v1, v2)
        } else {
            (v2, v1)
        }
    };

    let vertex_left = builder.add_vertex(FillVertex {
        position: vl.0,
        normal: vl.1,
    }).unwrap();

    let vertex_right = builder.add_vertex(FillVertex {
        position: vr.0,
        normal: vr.1,
    }).unwrap();

    GeometryBuilder::<FillVertex>::add_triangle(
        builder,
        last_vertex_left,
        last_vertex_right,
        vertex_left
    );
    GeometryBuilder::<FillVertex>::add_triangle(
        builder,
        last_vertex_right,
        vertex_right,
        vertex_left
    );

    GeometryBuilder::<FillVertex>::end_geometry(builder);
}