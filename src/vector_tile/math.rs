use std::f32::consts::PI;

fn deg2rad(deg: f32) -> f32 {
    2.0 * PI * deg / 360.0
}

fn rad2deg(rad: f32) -> f32 {
    360.0 * rad / (2.0 * PI)
}

pub fn deg2num(lat_deg: f32, lon_deg: f32, zoom: u32) -> (u32, u32) {
    let lat_rad = deg2rad(lat_deg);
    let n = 2f32.powi(zoom as i32);
    let xtile = ((lon_deg + 180.0) / 360.0 * n) as u32;
    let ytile = (
        (
            1.0 - (
                lat_rad.tan() + 1.0 / lat_rad.cos()
                  ).ln() / PI
        ) / 2.0 * n
    ) as u32;

    (xtile, ytile)
}

pub fn num2deg(xtile: u32, ytile: u32, zoom: u32) -> (f32, f32) {
    let n = 2f32.powi(zoom as i32);
    let lon_deg = xtile as f32 / n * 360.0 - 180.0;
    let lat_rad = ((PI * (1f32 - 2f32 * ytile as f32 / n)).sinh()).atan();
    let lat_deg = rad2deg(lat_rad);
    (lat_deg, lon_deg)
}