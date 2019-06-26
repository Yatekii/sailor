use lyon::math::{
    point,
    Point,

};

use lyon::path::{
    Path,
};

use lyon::tessellation::{
    VertexBuffers,
    FillOptions,
    FillTessellator,
    FillVertex,
    geometry_builder::simple_builder,
};

fn main() {
    let mut path_builder = Path::builder();
    path_builder.move_to(point(0.0, 0.0));
    path_builder.line_to(point(1.0, 2.0));
    path_builder.line_to(point(2.0, 0.0));
    path_builder.line_to(point(1.0, 1.0));
    path_builder.close();
    let path = path_builder.build();

    // Create the destination vertex and index buffers.
    let mut buffers: VertexBuffers<FillVertex, u16> = VertexBuffers::new();

    {
        let mut vertex_builder = simple_builder(&mut buffers);

        // Create the tessellator.
        let mut tessellator = FillTessellator::new();

        // Compute the tessellation.
        let result = tessellator.tessellate_path(
            path.iter(),
            &FillOptions::default(),
            &mut vertex_builder
        );
        assert!(result.is_ok());
    }

    println!("The generated vertices are: {:#?}.", &buffers.vertices[..]);
    println!("The generated indices are: {:#?}.", &buffers.indices[..]);
}