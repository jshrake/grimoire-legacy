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
    #[serde(skip)]
    ok: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum ResourceConfig {
    Image(ImageConfig),
    Texture2D(Texture2DConfig),
    Texture3D(Texture3DConfig),
    Cubemap(CubemapConfig),
    Video(VideoConfig),
    WebCam(WebCamConfig),
    Keyboard(KeyboardConfig),
    Audio(AudioConfig),
    Microphone(MicrophoneConfig),
    GstAppSinkPipeline(GstVideoPipelineConfig),
    Buffer(BufferConfig),
    UniformFloat(UniformFloatConfig),
    UniformVec2(UniformVec2Config),
    UniformVec3(UniformVec3Config),
    UniformVec4(UniformVec4Config),
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
pub struct UniformFloatConfig {
    pub uniform: f32,
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
pub struct UniformVec2Config {
    pub uniform: [f32; 2],
    pub min: [f32; 2],
    pub max: [f32; 2],
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
pub struct UniformVec3Config {
    pub uniform: [f32; 3],
    pub min: [f32; 3],
    pub max: [f32; 3],
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
pub struct UniformVec4Config {
    pub uniform: [f32; 4],
    pub min: [f32; 4],
    pub max: [f32; 4],
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct ImageConfig {
    pub image: String,
    #[serde(default = "default_flipv")]
    pub flipv: bool,
    #[serde(default)]
    pub fliph: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum TextureFormat {
    RU8,
    RF16,
    RF32,
    RGU8,
    RGF16,
    RGF32,
    RGBU8,
    RGBF16,
    RGBF32,
    RGBAU8,
    RGBAF16,
    RGBAF32,
    BGRU8,
    BGRF16,
    BGRF32,
    BGRAU8,
    BGRAF16,
    BGRAF32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Texture2DConfig {
    #[serde(rename = "texture2D")]
    pub texture_2d: String,
    pub width: u32,
    pub height: u32,
    pub format: TextureFormat,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct Texture3DConfig {
    #[serde(rename = "texture3D")]
    pub texture_3d: String,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub format: TextureFormat,
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
    // render pass settings
    pub buffer: Option<String>,
    pub clear: Option<[f32; 4]>,
    pub blend: Option<BlendConfig>,
    pub depth: Option<DepthFuncConfig>,
    #[serde(default)]
    pub disable: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct BufferConfig {
    pub buffer: bool,
    #[serde(default = "default_buffer_config_attachments")]
    pub attachments: usize,
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
    Simple(String),
    Complete {
        resource: String,
        #[serde(default)]
        attachment: usize,
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
    Mipmap,
}

impl ChannelConfig {
    pub fn resource_name(&self) -> &String {
        match self {
            ChannelConfig::Simple(s) => s,
            ChannelConfig::Complete { resource, .. } => &resource,
        }
    }
}

impl EffectConfig {
    pub fn from_toml(src_str: &str) -> Result<EffectConfig> {
        toml::from_str(src_str)
            .map_err(Error::toml)
            .map(|mut c: EffectConfig| {
                c.validate().unwrap();
                Ok(c)
            })?
    }

    pub fn is_ok(&self) -> bool {
        self.ok
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

    fn validate(&mut self) -> Result<()> {
        // check that the buffer names reference valid resources
        self.ok = true;
        let resource_names = &self
            .resources
            .iter()
            .filter(|(_, r)| match r {
                ResourceConfig::UniformFloat(_)
                | ResourceConfig::UniformVec2(_)
                | ResourceConfig::UniformVec3(_)
                | ResourceConfig::UniformVec4(_) => false,
                _ => true,
            }).map(|(k, _)| k.as_str())
            .collect::<Vec<&str>>();
        let buffer_names = &self
            .resources
            .iter()
            .filter(|(_, r)| match r {
                ResourceConfig::Buffer(_) => true,
                _ => false,
            }).map(|(k, _)| k.as_str())
            .collect::<Vec<&str>>();

        // Validate buffer names
        for (pass_index, pass) in self.passes.iter().enumerate() {
            if let Some(ref buffer) = pass.buffer {
                if !self.resources.contains_key(buffer) {
                    self.ok = false;
                    error!(
                        "[TOML] Could not find buffer referenced in pass {} with name \"{}\". Valid buffer names: {:?}",
                        pass_index, buffer,buffer_names
                    );
                }
            }
        }

        // Validate resource names
        for (pass_index, pass) in self.passes.iter().enumerate() {
            for (uniform_name, channel_config) in &pass.uniform_to_channel {
                let resource_name = channel_config.resource_name();
                if !self.resources.contains_key(resource_name) {
                    self.ok = false;
                    error!(
                        "[TOML] Could not find resource referenced in pass {}, {}=\"{}\". Valid resource names: {:?}",
                        pass_index, uniform_name, resource_name, resource_names
                    );
                }
            }
        }

        // Validate that all pass resource references are not uniform inputs
        for (pass_index, pass) in self.passes.iter().enumerate() {
            for (uniform_name, channel_config) in &pass.uniform_to_channel {
                let resource_name = channel_config.resource_name();
                match self.resources[resource_name] {
                    ResourceConfig::UniformFloat(_)
                    | ResourceConfig::UniformVec2(_)
                    | ResourceConfig::UniformVec3(_)
                    | ResourceConfig::UniformVec4(_) => {
                        self.ok = false;
                        error!(
                        "[TOML] Cannot reference uniform in pass {}, {}=\"{}\". Valid resource names: {:?}",
                        pass_index, uniform_name, resource_name, resource_names
                    );
                    }
                    _ => (),
                }
            }
        }

        // Validate buffer configuration
        for (resource_name, resource_config) in &self.resources {
            if let ResourceConfig::Buffer(buffer) = resource_config {
                // unwrap into dummy values of 1 if not present
                // we simply want to check if the user set these to 0
                let buffer_width = buffer.width.unwrap_or(1);
                let buffer_height = buffer.height.unwrap_or(1);
                let attachments = buffer.attachments;
                if buffer_width == 0 || buffer_height == 0 {
                    self.ok = false;
                    error!(
                            "[TOML] Buffer \"{}\" must specify non-zero value for the width and height properties",
                            resource_name
                        );
                }
                if attachments == 0 {
                    self.ok = false;
                    error!(
                            "[TOML] Buffer \"{}\" must specify non-zero value for the attachments property",
                            resource_name
                        );
                }
            }
        }

        Ok(())
    }
}

impl Default for WrapConfig {
    fn default() -> Self {
        WrapConfig::Repeat
    }
}

impl Default for FilterConfig {
    fn default() -> Self {
        FilterConfig::Mipmap
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
            buffer: true,
            attachments: 1,
            format: BufferFormat::F32,
            width: None,
            height: None,
        }
    }
}

impl TextureFormat {
    pub fn channels(&self) -> usize {
        match self {
            TextureFormat::RU8 | TextureFormat::RF16 | TextureFormat::RF32 => 1,
            TextureFormat::RGU8 | TextureFormat::RGF16 | TextureFormat::RGF32 => 2,
            TextureFormat::RGBU8 | TextureFormat::RGBF16 | TextureFormat::RGBF32 => 3,
            TextureFormat::BGRU8 | TextureFormat::BGRF16 | TextureFormat::BGRF32 => 3,
            TextureFormat::RGBAU8 | TextureFormat::RGBAF16 | TextureFormat::RGBAF32 => 4,
            TextureFormat::BGRAU8 | TextureFormat::BGRAF16 | TextureFormat::BGRAF32 => 4,
        }
    }
    pub fn bytes_per(&self) -> usize {
        let c = self.channels();
        match self {
            TextureFormat::RU8 => c,
            TextureFormat::RF16 => c * 2,
            TextureFormat::RF32 => c * 3,
            TextureFormat::RGU8 => c * 2,
            TextureFormat::RGF16 => c * 2 * 2,
            TextureFormat::RGF32 => c * 2 * 3,
            TextureFormat::RGBU8 => c * 3,
            TextureFormat::RGBF16 => c * 3 * 2,
            TextureFormat::RGBF32 => c * 3 * 3,
            TextureFormat::RGBAU8 => c * 4,
            TextureFormat::RGBAF16 => c * 4 * 2,
            TextureFormat::RGBAF32 => c * 4 * 3,
            TextureFormat::BGRU8 => c * 3,
            TextureFormat::BGRF16 => c * 3 * 2,
            TextureFormat::BGRF32 => c * 3 * 3,
            TextureFormat::BGRAU8 => c * 4,
            TextureFormat::BGRAF16 => c * 4 * 2,
            TextureFormat::BGRAF32 => c * 4 * 3,
        }
    }
}

fn default_audio_bands() -> usize {
    512
}

fn default_flipv() -> bool {
    true
}

fn default_buffer_config_attachments() -> usize {
    1
}

fn default_buffer_config_format() -> BufferFormat {
    BufferFormat::F32
}
