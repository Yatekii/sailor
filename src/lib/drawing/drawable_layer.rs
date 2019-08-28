use core::ops::Range;

#[derive(Debug, Clone)]
pub struct DrawableLayer {
    pub id: u32,
    pub indices_range: Range<u32>,
    pub features: Vec<(u32, Range<u32>)>
}