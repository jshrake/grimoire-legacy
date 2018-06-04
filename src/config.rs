use std::collections::BTreeMap;

use error::{Error, Result};
use regex::Regex;
use toml;

#[derive(Debug, Default, Deserialize, PartialEq, Clone)]
pub struct EffectConfig {
    #[serde(rename = "pass", default)]
    pub passes: Vec<PassConfig>,
    #[serde(flatten, default)]
    pub resources: BTreeMap<String, ResourceConfig>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum ResourceConfig {
    Image(ImageConfig),
    Texture3D(Texture3DConfig),
    Cubemap(CubemapConfig),
    Video(VideoConfig),
    WebCam(WebCamConfig),
    Keyboard(KeyboardConfig),
    Audio(AudioConfig),
    Microphone(MicrophoneConfig),
    GstAppSinkPipeline(GstVideoPipelineConfig),
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct ImageConfig {
    pub image: String,
    #[serde(default = "default_flipv")]
    pub flipv: bool,
    #[serde(default)]
    pub fliph: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Texture3DConfig {
    #[serde(rename = "texture3D")]
    pub texture_3d: String,
    pub resolution: [u32; 3],
    pub components: u32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct CubemapConfig {
    pub right: String,
    pub left: String,
    pub top: String,
    pub bottom: String,
    pub back: String,
    pub front: String,
    #[serde(default = "default_flipv")]
    pub flipv: bool,
    #[serde(default)]
    pub fliph: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct VideoConfig {
    pub video: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct WebCamConfig {
    pub webcam: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct KeyboardConfig {
    pub keyboard: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct AudioConfig {
    pub audio: String,
    #[serde(default = "default_audio_bands")]
    pub bands: usize,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct GstVideoPipelineConfig {
    pub pipeline: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct MicrophoneConfig {
    pub microphone: bool,
    #[serde(default = "default_audio_bands")]
    pub bands: usize,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct PassConfig {
    #[serde(default)]
    pub draw: DrawConfig,
    #[serde(flatten)]
    pub uniform_to_channel: BTreeMap<String, ChannelConfig>,
    #[serde(default)]
    pub buffer: BufferConfig,
    // render pass settings
    #[serde(default = "default_clear")]
    pub clear: [f32; 4],
    pub blend: Option<BlendConfig>,
    pub depth: Option<DepthFuncConfig>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct BufferConfig {
    #[serde(default = "default_buffer_config_attachments")]
    pub attachments: u32,
    #[serde(default = "default_buffer_config_format")]
    pub format: BufferFormat,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum BufferFormat {
    U8,
    F16,
    F32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DrawConfig {
    pub mode: DrawModeConfig,
    pub count: u32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub enum DepthFuncConfig {
    #[serde(rename = "never")]
    Never,
    #[serde(rename = "less")]
    Less,
    #[serde(rename = "equal")]
    Equal,
    #[serde(rename = "less-equal")]
    LEqual,
    #[serde(rename = "greater")]
    Greater,
    #[serde(rename = "not-equal")]
    NotEqual,
    #[serde(rename = "greater-equal")]
    GEqual,
    #[serde(rename = "always")]
    Always,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct BlendConfig {
    pub src: BlendFactorConfig,
    pub dst: BlendFactorConfig,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub enum BlendFactorConfig {
    #[serde(rename = "zero")]
    Zero,
    #[serde(rename = "one")]
    One,
    #[serde(rename = "src-color")]
    SrcColor,
    #[serde(rename = "one-minus-src-color")]
    OneMinusSrcColor,
    #[serde(rename = "dst-color")]
    DstColor,
    #[serde(rename = "one-minus-dst-color")]
    OneMinusDstColor,
    #[serde(rename = "src-alpha")]
    SrcAlpha,
    #[serde(rename = "one-minus-src-alpha")]
    OneMinusSrcAlpha,
    #[serde(rename = "dst-alpha")]
    DstAlpha,
    #[serde(rename = "one-minus-dst-alpha")]
    OneMinusDstAlpha,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub enum DrawModeConfig {
    #[serde(rename = "triangles")]
    Triangles,
    #[serde(rename = "triangle-fan")]
    TriangleFan,
    #[serde(rename = "triangle-strip")]
    TriangleStrip,
    #[serde(rename = "lines")]
    Lines,
    #[serde(rename = "line-strip")]
    LineStrip,
    #[serde(rename = "line-loop")]
    LineLoop,
    #[serde(rename = "points")]
    Points,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum ChannelConfig {
    SimplePass(u32),
    SimplePassAttachment([u32; 2]),
    CompletePass {
        pass: u32,
        #[serde(default)]
        attachment: u32,
        #[serde(default)]
        wrap: WrapConfig,
        #[serde(default)]
        filter: FilterConfig,
    },
    SimpleResource(String),
    CompleteResource {
        resource: String,
        #[serde(default)]
        wrap: WrapConfig,
        #[serde(default)]
        filter: FilterConfig,
    },
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum WrapConfig {
    Clamp,
    Repeat,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub enum FilterConfig {
    Linear,
    Nearest,
}

impl Default for WrapConfig {
    fn default() -> Self {
        WrapConfig::Repeat
    }
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig::Linear
    }
}

impl Default for DrawConfig {
    fn default() -> Self {
        Self {
            mode: DrawModeConfig::Triangles,
            count: 1,
        }
    }
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            attachments: 1,
            format: BufferFormat::F32,
            width: None,
            height: None,
        }
    }
}

impl EffectConfig {
    pub fn from_toml(src_str: &str) -> Result<EffectConfig> {
        toml::from_str(src_str).map_err(|err| Error::toml(err))
    }

    pub fn from_comment_block_in_str(src: &str) -> Result<EffectConfig> {
        lazy_static! {
            static ref FIRST_COMMENT_BLOCK_RE: Regex =
                Regex::new(r"(?s)/\*(?P<config>.*?)\*/").expect("failed to compile config_regex");
        }
        if let Some(caps) = FIRST_COMMENT_BLOCK_RE.captures(&src) {
            let config_str = caps
                .name("config")
                .expect("config_from_comment_block: could not find config capture group")
                .as_str();
            Ok(EffectConfig::from_toml(config_str)?)
        } else {
            Ok(EffectConfig::from_toml("[[pass]]")?)
        }
    }
}

fn default_clear() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
}

fn default_audio_bands() -> usize {
    512
}

fn default_flipv() -> bool {
    true
}

fn default_buffer_config_attachments() -> u32 {
    1
}

fn default_buffer_config_format() -> BufferFormat {
    BufferFormat::F32
}
