use crate::drawing::mesh::MeshBuilder;
use crate::lyon::lyon_tessellation::GeometryBuilder;
use lyon::path::Path;
use lyon::math::*;
use lyon::tessellation::{
    FillVertex,
};

const LINE_WIDTH: f32 = 12.0;

pub fn between(a: &Vector, b: &Vector, c: &Vector) -> bool {
    num_traits::sign::signum(a.y * b.x - a.x * b.y) != num_traits::sign::signum(a.y * c.x - a.x * c.y)
}

pub fn tesselate_line2<'a, 'l>(path: &Path, builder: &'a mut MeshBuilder<'l>) {
    GeometryBuilder::<FillVertex>::begin_geometry(builder);
    // Fill
    builder.set_current_vertex_type(false);
    let points = path.points();

    let mut last_line = points[1].to_vector() - points[0].to_vector();
    let normal = vector(last_line.y, -last_line.x);
    let mut last_normal = if points.len() > 2 {
        let nl = points[2].to_vector() - points[1].to_vector();
        let dot = normal.dot(last_line.normalize() + nl.normalize());
        if dot <= 1.0 && dot >= 0.0 {
            normal
        } else {
            -normal
        }
    } else {
        normal
    }.normalize() * LINE_WIDTH;

    let mut last_vertex_1 = builder.add_vertex(FillVertex {
        position: points[0] + last_normal,
        normal: last_normal,
    }).unwrap();

    let mut last_vertex_2 = builder.add_vertex(FillVertex {
        position: points[0] - last_normal,
        normal: -last_normal,
    }).unwrap();

    if points.len() > 2 {
        for i in 0..points.len() - 2 {
            let current_line = points[i + 1].to_vector() - points[i].to_vector();
            let next_line = points[i + 2].to_vector() - points[i + 1].to_vector();

            let normal = current_line.normalize() + next_line.normalize();
            let local_normal = vector(last_line.y, -last_line.x);
            let dot = local_normal.dot(last_normal);
            let local_normal = if dot <= 1.0 && dot >= 0.0 {
                local_normal
            } else {
                -local_normal
            };

            let normal = if normal.x == 0.0 && normal.y == 0.0 {
                local_normal
            } else {
                normal
            }.normalize() * LINE_WIDTH;

            let factor = normal.dot(local_normal) / (normal.length() * local_normal.length());

            let vertex_1 = builder.add_vertex(FillVertex {
                position: points[i] + normal * factor,
                normal: normal,
            }).unwrap();

            let vertex_2 = builder.add_vertex(FillVertex {
                position: points[i] - normal * factor,
                normal: - normal,
            }).unwrap();

            GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_1, last_vertex_2, vertex_1);
            GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_2, vertex_2, vertex_1);

            last_vertex_1 = vertex_1;
            last_vertex_2 = vertex_2;
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

    let vertex_1 = builder.add_vertex(FillVertex {
        position: points[points.len() - 1] + normal,
        normal: normal,
    }).unwrap();

    let vertex_2 = builder.add_vertex(FillVertex {
        position: points[points.len() - 1] - normal,
        normal: -normal,
    }).unwrap();

    GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_1, last_vertex_2, vertex_1);
    GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_2, vertex_2, vertex_1);

    GeometryBuilder::<FillVertex>::end_geometry(builder);
}

pub fn tesselate_line<'a, 'l>(path: &Path, builder: &'a mut MeshBuilder<'l>) {
    GeometryBuilder::<FillVertex>::begin_geometry(builder);
    // Fill
    builder.set_current_vertex_type(false);
    let points = path.points();

    let mut last_line = points[1].to_vector() - points[0].to_vector();
    let next_line = if points.len() > 2 {
        points[2].to_vector() - points[1].to_vector()
    } else {
        last_line
    };
    let mut last_normal: Vector = vector(last_line.y, -last_line.x).normalize() * 4.0;

    if between(&last_normal, &last_line, &next_line) {
        last_normal = -last_normal;
    }

    let mut last_vertex_1 = builder.add_vertex(FillVertex {
        position: points[0] + last_normal,
        normal: last_normal,
    }).unwrap();

    let mut last_vertex_2 = builder.add_vertex(FillVertex {
        position: points[0] - last_normal,
        normal: -last_normal,
    }).unwrap();

    for i in 1..points.len() - 1{
        let next_line = points[i + 1].to_vector() - points[i].to_vector();
        let mut next_normal: Vector = vector(next_line.y, -next_line.x).normalize() * 4.0;
        
        if between(&next_normal, &last_line, &next_line) {
            next_normal = -next_normal;
        }

        let mut normal = last_normal + next_normal;
        if normal.x == 0.0 && normal.y == 0.0 {
            normal = last_normal - next_normal;
        }

        let factor = normal.dot(last_normal) / (normal.length() * last_normal.length());

        let vertex_1 = builder.add_vertex(FillVertex {
            position: points[i] + normal * factor,
            normal: normal,
        }).unwrap();

        let vertex_2 = builder.add_vertex(FillVertex {
            position: points[i] - normal * factor,
            normal: - normal,
        }).unwrap();

        GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_1, last_vertex_2, vertex_1);
        GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_2, vertex_2, vertex_1);

        last_vertex_1 = vertex_1;
        last_vertex_2 = vertex_2;
        last_normal = next_normal;
        last_line = next_line;
    }

    let line = points[points.len() - 1].to_vector() - points[points.len() - 2].to_vector();
    let mut normal: Vector = vector(line.y, -line.x).normalize() * 4.0;

    if between(&normal, &last_line, &line) {
        normal = -normal;
    }

    let vertex_1 = builder.add_vertex(FillVertex {
        position: points[points.len() - 1] + normal,
        normal: normal,
    }).unwrap();

    let vertex_2 = builder.add_vertex(FillVertex {
        position: points[points.len() - 1] - normal,
        normal: -normal,
    }).unwrap();

    GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_1, last_vertex_2, vertex_1);
    GeometryBuilder::<FillVertex>::add_triangle(builder, last_vertex_2, vertex_2, vertex_1);

    GeometryBuilder::<FillVertex>::end_geometry(builder);
}