use crate::config::TextureFormat;
use std::fmt;

#[derive(Debug)]
pub enum ResourceData {
    Geometry(GeometryData),
    D2(ResourceData2D),
    D3(ResourceData3D),
    Cube(Vec<(ResourceCubemapFace, ResourceData2D)>),
}

#[derive(Debug)]
pub struct GeometryData {
    pub buffer: Vec<f32>,
    pub pos_stride_off: (u32, u32), // Assumes vec3
    pub nrm_stride_off: (u32, u32), // Assumes vec3
}

#[derive(Debug)]
pub struct ResourceData2D {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
    // subtex params
    pub xoffset: u32,
    pub yoffset: u32,
    pub subwidth: u32,
    pub subheight: u32,
    // additional uniform data
    pub time: f32,
}

#[derive(Debug)]
pub struct ResourceData3D {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub format: TextureFormat,
    // subtex params
    pub xoffset: u32,
    pub yoffset: u32,
    pub zoffset: u32,
    pub subwidth: u32,
    pub subheight: u32,
    pub subdepth: u32,
    // additional uniform data
    pub time: f32,
}

#[derive(Debug, Copy, Clone)]
pub enum ResourceCubemapFace {
    Right,
    Left,
    Top,
    Bottom,
    Front,
    Back,
}

impl fmt::Display for ResourceData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ResourceData::D2(data) => write!(
                f,
                "Texture2D(width={}, height={}, format={:?})",
                data.width, data.height, data.format
            ),
            ResourceData::D3(data) => write!(
                f,
                "Texture3D(width={}, height={}, depth={}, format={:?})",
                data.width, data.height, data.depth, data.format
            ),
            ResourceData::Cube(faces) => write!(f, "TextureCubemap({:?})", faces),
            _ => fmt::Result::Ok(()),
        }
    }
}
