use std::fmt;

#[derive(Debug)]
pub enum ResourceData {
    D2(ResourceData2D),
    D3(ResourceData3D),
    Cube(Vec<(ResourceCubemapFace, ResourceData2D)>),
}

#[derive(Debug)]
pub struct ResourceData2D {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub channels: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub subwidth: u32,
    pub subheight: u32,
    pub time: f32,
    pub kind: ResourceDataKind,
}

#[derive(Debug)]
pub struct ResourceData3D {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub channels: u32,
    pub time: f32,
    pub kind: ResourceDataKind,
}

#[derive(Debug, Copy, Clone)]
pub enum ResourceDataKind {
    U8,
    F16,
    F32,
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
                "Texture2D(width={}, height={}, channels={}, kind={:?})",
                data.width, data.height, data.channels, data.kind,
            ),
            ResourceData::D3(data) => write!(
                f,
                "Texture3D(width={}, height={}, depth={}, channels={}, kind={:?})",
                data.width, data.height, data.depth, data.channels, data.kind
            ),
            ResourceData::Cube(faces) => write!(f, "TextureCubemap({:?})", faces),
        }
    }
}
