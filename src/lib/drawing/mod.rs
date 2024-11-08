pub mod line_tesselator;
pub mod loaded_gpu_tile;
pub mod mesh;
pub mod vertex;

pub fn as_byte_slice<T>(slice: &[T]) -> &[u8] {
    let len = std::mem::size_of_val(slice);
    let ptr = slice.as_ptr() as *const u8;
    unsafe { std::slice::from_raw_parts(ptr, len) }
}
