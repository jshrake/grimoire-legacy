use std;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryIter, TryRecvError};
use std::time::Duration;

use audio::Audio;
use config::{ResourceConfig, TextureFormat};
use error::{Error, Result};
use image;
use image::GenericImage;
use keyboard::Keyboard;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use platform::Platform;
use resource::{ResourceCubemapFace, ResourceData, ResourceData2D, ResourceData3D};
use video::Video;

pub struct ResourceStream {
    pub sender: ResourceSender,
    pub receiver: ResourceReceiver,
    pub ctx: Option<ResourceStreamCtx>,
    pub watch: Option<ResourceWatch>,
    pub name: String,
}

pub enum ResourceStreamCtx {
    Keyboard(Keyboard),
    Video(Video),
    Audio(Audio),
}

pub struct ResourceWatch {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    config: ResourceConfig,
    rx: Receiver<DebouncedEvent>,
    force_read: bool,
}

pub trait Stream {
    fn play(&mut self) -> Result<()> {
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn restart(&mut self) -> Result<()> {
        Ok(())
    }
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()>;
}

type ResourceSender = Sender<ResourceData>;
type ResourceReceiver = Receiver<ResourceData>;

impl ResourceStream {
    pub fn new(name: &str, config: &ResourceConfig) -> Result<Self> {
        let (tx, rx) = channel();
        let ctx = match config {
            ResourceConfig::Video(config) => {
                let uri = PathBuf::from(&config.video);
                let uri = uri
                    .canonicalize()
                    .map(|r| ["file://", r.to_str().unwrap()].concat())
                    .unwrap_or_else(|_| config.video.clone());
                let mut video = Video::new_video(&uri)?;
                video.play()?;
                Some(ResourceStreamCtx::Video(video))
            }
            ResourceConfig::WebCam(_config) => {
                let mut webcam = Video::new_webcam()?;
                webcam.play()?;
                Some(ResourceStreamCtx::Video(webcam))
            }
            ResourceConfig::Audio(config) => {
                let uri = PathBuf::from(&config.audio);
                let uri = uri
                    .canonicalize()
                    .map(|r| ["file://", r.to_str().unwrap()].concat())
                    .unwrap_or_else(|_| config.audio.clone());
                let mut audio = Audio::new_audio(&uri, config.bands)?;
                audio.play()?;
                Some(ResourceStreamCtx::Audio(audio))
            }
            ResourceConfig::Microphone(config) => {
                let mut microphone = Audio::new_microphone(config.bands)?;
                microphone.play()?;
                Some(ResourceStreamCtx::Audio(microphone))
            }
            ResourceConfig::GstAppSinkPipeline(config) => {
                let mut video = Video::new_appsink_pipeline(&config.pipeline)?;
                video.play()?;
                Some(ResourceStreamCtx::Video(video))
            }
            ResourceConfig::Keyboard(config) => {
                Some(ResourceStreamCtx::Keyboard(Keyboard::new(config)))
            }
            _ => None,
        };
        // watch channel
        let watch = ResourceWatch::from_config(config.clone())?;
        Ok(ResourceStream {
            sender: tx,
            receiver: rx,
            ctx,
            watch: Some(watch),
            name: name.to_string(),
        })
    }

    pub fn tick(&mut self, platform: &mut Platform) -> Result<TryIter<ResourceData>> {
        if let Some(ref mut ctx) = self.ctx {
            if let ResourceStreamCtx::Keyboard(ref mut keyboard) = ctx {
                keyboard.tick(&platform.events.keyboard_state());
            }
        }
        let sender = self.sender.clone();
        self.stream_to(&sender)?;
        Ok(self.receiver.try_iter())
    }
}

impl ResourceWatch {
    fn from_config(config: ResourceConfig) -> Result<Self> {
        // helper function
        let watch_path = |watcher: &mut RecommendedWatcher, path: &str| -> Result<()> {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|err| Error::watch_path(path, err))?;
            Ok(())
        };
        let (tx, rx) = channel();
        let mut watcher: RecommendedWatcher =
            Watcher::new(tx, Duration::from_millis(200)).map_err(Error::notify)?;
        match config {
            ResourceConfig::Image(ref config) => {
                watch_path(&mut watcher, &config.image)?;
            }
            ResourceConfig::Texture3D(ref config) => {
                watch_path(&mut watcher, &config.texture_3d)?;
            }
            ResourceConfig::Texture2D(ref config) => {
                watch_path(&mut watcher, &config.texture_2d)?;
            }
            ResourceConfig::Video(ref config) => {
                if Path::new(&config.video).exists() {
                    watch_path(&mut watcher, &config.video)?;
                }
            }
            ResourceConfig::Audio(ref config) => {
                if Path::new(&config.audio).exists() {
                    watch_path(&mut watcher, &config.audio)?;
                }
            }
            ResourceConfig::Cubemap(ref config) => {
                watch_path(&mut watcher, &config.left)?;
                watch_path(&mut watcher, &config.right)?;
                watch_path(&mut watcher, &config.front)?;
                watch_path(&mut watcher, &config.back)?;
                watch_path(&mut watcher, &config.top)?;
                watch_path(&mut watcher, &config.bottom)?;
            }
            ResourceConfig::WebCam(_) => (),
            ResourceConfig::Microphone(_) => (),
            ResourceConfig::Keyboard(_) => (),
            ResourceConfig::GstAppSinkPipeline(_) => (),
            ResourceConfig::Buffer(_) => (),
            ResourceConfig::UniformFloat(_) => (),
            ResourceConfig::UniformVec2(_) => (),
            ResourceConfig::UniformVec3(_) => (),
            ResourceConfig::UniformVec4(_) => (),
        }
        Ok(ResourceWatch {
            watcher,
            config,
            rx,
            force_read: true,
        })
    }
}

impl Stream for ResourceStream {
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        if let Some(ref mut watch) = self.watch {
            watch.stream_to(dest)?;
        }
        if let Some(ref mut ctx) = self.ctx {
            ctx.stream_to(dest)?;
        }
        Ok(())
    }

    fn play(&mut self) -> Result<()> {
        self.ctx.as_mut().map(|ctx| ctx.play()).unwrap_or(Ok(()))
    }

    fn pause(&mut self) -> Result<()> {
        self.ctx.as_mut().map(|ctx| ctx.pause()).unwrap_or(Ok(()))
    }

    fn restart(&mut self) -> Result<()> {
        self.ctx.as_mut().map(|ctx| ctx.restart()).unwrap_or(Ok(()))
    }
}

impl Stream for ResourceStreamCtx {
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        match self {
            ResourceStreamCtx::Video(ref mut s) => s.stream_to(dest),
            ResourceStreamCtx::Audio(ref mut s) => s.stream_to(dest),
            ResourceStreamCtx::Keyboard(ref mut s) => s.stream_to(dest),
        }
    }

    fn play(&mut self) -> Result<()> {
        match self {
            ResourceStreamCtx::Video(ref mut s) => s.play(),
            ResourceStreamCtx::Audio(ref mut s) => s.play(),
            _ => Ok(()),
        }
    }

    fn pause(&mut self) -> Result<()> {
        match self {
            ResourceStreamCtx::Video(ref mut s) => s.pause(),
            ResourceStreamCtx::Audio(ref mut s) => s.pause(),
            _ => Ok(()),
        }
    }

    fn restart(&mut self) -> Result<()> {
        match self {
            ResourceStreamCtx::Video(ref mut s) => s.restart(),
            ResourceStreamCtx::Audio(ref mut s) => s.restart(),
            _ => Ok(()),
        }
    }
}

impl Stream for ResourceWatch {
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let event = self.rx.try_recv();
        let should_read = match event {
            Ok(DebouncedEvent::Write(_)) | Ok(DebouncedEvent::Create(_)) => true,
            Ok(_) | Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                return Err(Error::bug(
                    "ResourceWatch::stream_to rx.try_recv got unexpected disconnect",
                ));
            }
        };
        if self.force_read || should_read {
            self.force_read = false;
            if let Some(resource) = resource_from_config(&self.config)? {
                dest.send(resource).map_err(|err| {
                    Error::bug(format!(
                        "ResourceWatch::stream_to dest.send failed: {}",
                        err
                    ))
                })?;
            }
        }
        Ok(())
    }
}

fn resource_from_config(config: &ResourceConfig) -> Result<Option<ResourceData>> {
    match config {
        ResourceConfig::Image(config) => {
            let mut image =
                image::open(&config.image).map_err(|err| Error::image(&config.image, err))?;
            if config.flipv {
                image = image.flipv();
            }
            if config.fliph {
                image = image.fliph();
            }
            let format = match image {
                image::DynamicImage::ImageLuma8(_) => TextureFormat::RU8,
                image::DynamicImage::ImageLumaA8(_) => TextureFormat::RGU8,
                image::DynamicImage::ImageRgb8(_) => TextureFormat::RGBU8,
                image::DynamicImage::ImageRgba8(_) => TextureFormat::RGBAU8,
            };
            let (width, height) = image.dimensions();
            Ok(Some(ResourceData::D2(ResourceData2D {
                bytes: image.raw_pixels(),
                width,
                height,
                format,
                xoffset: 0,
                yoffset: 0,
                subwidth: width,
                subheight: height,
                time: 0.0,
            })))
        }
        ResourceConfig::Cubemap(config) => {
            // build cube maps
            let image_paths = &[
                (ResourceCubemapFace::Right, &config.right),
                (ResourceCubemapFace::Left, &config.left),
                (ResourceCubemapFace::Top, &config.top),
                (ResourceCubemapFace::Bottom, &config.bottom),
                (ResourceCubemapFace::Front, &config.front),
                (ResourceCubemapFace::Back, &config.back),
            ];
            let mut cubemap = Vec::new();
            for &(ref face, ref path) in image_paths.iter() {
                let mut image = image::open(&path).map_err(|err| Error::image(path, err))?;
                if config.flipv {
                    image = image.flipv();
                }
                if config.fliph {
                    image = image.fliph();
                }
                // TODO(jshrake): Determine the native channels
                // and size values to use rather than hard coding RGB8
                let format = match image {
                    image::DynamicImage::ImageLuma8(_) => TextureFormat::RU8,
                    image::DynamicImage::ImageLumaA8(_) => TextureFormat::RGU8,
                    image::DynamicImage::ImageRgb8(_) => TextureFormat::RGBU8,
                    image::DynamicImage::ImageRgba8(_) => TextureFormat::RGBAU8,
                };
                let (width, height) = image.dimensions();
                let resource = ResourceData2D {
                    bytes: image.raw_pixels(),
                    width,
                    height,
                    format,
                    xoffset: 0,
                    yoffset: 0,
                    subwidth: width,
                    subheight: height,
                    time: 0.0,
                };
                cubemap.push((*face, resource));
            }
            Ok(Some(ResourceData::Cube(cubemap)))
        }
        ResourceConfig::Texture2D(config) => {
            let f = std::fs::File::open(&config.texture_2d)
                .map_err(|err| Error::io(&config.texture_2d, err))?;
            let mut reader = std::io::BufReader::new(f);
            let mut bytes = Vec::new();
            let read = reader
                .read_to_end(&mut bytes)
                .map_err(|err| Error::io(&config.texture_2d, err))?;
            let width = config.width;
            let height = config.height;
            assert!(read as u32 == width * height * config.format.channels() as u32);
            Ok(Some(ResourceData::D2(ResourceData2D {
                bytes,
                width,
                height,
                format: config.format,
                subwidth: width,
                subheight: height,
                xoffset: 0,
                yoffset: 0,
                time: 0.0,
            })))
        }
        ResourceConfig::Texture3D(config) => {
            let f = std::fs::File::open(&config.texture_3d)
                .map_err(|err| Error::io(&config.texture_3d, err))?;
            let mut reader = std::io::BufReader::new(f);
            let mut bytes = Vec::new();
            let read = reader
                .read_to_end(&mut bytes)
                .map_err(|err| Error::io(&config.texture_3d, err))?;
            let width = config.width;
            let height = config.height;
            let depth = config.depth;
            assert!(read as u32 == width * height * depth * config.format.channels() as u32);
            Ok(Some(ResourceData::D3(ResourceData3D {
                bytes,
                width,
                height,
                depth,
                format: config.format,
                subwidth: width,
                subheight: height,
                subdepth: depth,
                xoffset: 0,
                yoffset: 0,
                zoffset: 0,
                time: 0.0,
            })))
        }
        ResourceConfig::Video(_) => Ok(None),
        ResourceConfig::WebCam(_) => Ok(None),
        ResourceConfig::Audio(_) => Ok(None),
        ResourceConfig::Microphone(_) => Ok(None),
        ResourceConfig::Keyboard(_) => Ok(None),
        ResourceConfig::GstAppSinkPipeline(_) => Ok(None),
        ResourceConfig::Buffer(_) => Ok(None),
        ResourceConfig::UniformFloat(_) => Ok(None),
        ResourceConfig::UniformVec2(_) => Ok(None),
        ResourceConfig::UniformVec3(_) => Ok(None),
        ResourceConfig::UniformVec4(_) => Ok(None),
    }
}
