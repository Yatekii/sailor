mod line_tesselator;
mod loaded_gpu_tile;
mod mesh;
mod vertex;

pub use line_tesselator::*;
pub use loaded_gpu_tile::*;
pub use mesh::*;
pub use vertex::*;

pub fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    let len = slice.len() * std::mem::size_of::<T>();
    let ptr = slice.as_ptr() as *const u8;
    unsafe { std::slice::from_raw_parts(ptr, len) }
}
