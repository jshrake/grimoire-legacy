use std;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryIter, TryRecvError};
use std::time::Duration;

use audio::Audio;
use config::ResourceConfig;
use error::{Error, Result};
use image;
use image::GenericImage;
use keyboard::Keyboard;
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use platform::Platform;
use resource::{
    ResourceCubemapFace, ResourceData, ResourceData2D, ResourceData3D, ResourceDataKind,
};
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
    force_load: bool,
}

pub trait Stream {
    fn play(&mut self) -> Result<()> {
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
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
                    .unwrap_or(config.video.clone());
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
                    .unwrap_or(config.audio.clone());
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
            ctx: ctx,
            watch: Some(watch),
            name: name.to_string(),
        })
    }

    pub fn tick(&mut self, platform: &mut Platform) -> Result<TryIter<ResourceData>> {
        if let Some(ref mut ctx) = self.ctx {
            match ctx {
                ResourceStreamCtx::Keyboard(ref mut keyboard) => {
                    keyboard.tick(&platform.events.keyboard_state())
                }
                _ => {}
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
            Watcher::new(tx, Duration::from_millis(200)).map_err(|err| Error::notify(err))?;
        match config {
            ResourceConfig::Image(ref config) => {
                watch_path(&mut watcher, &config.image)?;
            }
            ResourceConfig::Texture3D(ref config) => {
                watch_path(&mut watcher, &config.texture_3d)?;
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
        }
        Ok(ResourceWatch {
            watcher: watcher,
            config: config,
            rx: rx,
            force_load: true,
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
}

impl Stream for ResourceWatch {
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let event = self.rx.try_recv();
        let should_read = match event {
            Ok(DebouncedEvent::Write(_)) | Ok(DebouncedEvent::Create(_)) => true,
            Ok(_) | Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                panic!("ResourceWatch::stream_to rx.try_recv failed due to unexpected disconnect.\nSee https://doc.rust-lang.org/std/sync/mpsc/enum.TryRecvError.html");
            }
        };
        if self.force_load || should_read {
            self.force_load = false;
            if let Some(resource) = resource_from_config(&self.config)? {
                dest.send(resource).expect("ResourceWatch::stream_to dest.send failed due to unexpected disconnect.\nSee https://doc.rust-lang.org/std/sync/mpsc/struct.SendError.html");
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
            let channel_count = match image {
                image::DynamicImage::ImageLuma8(_) => 1,
                image::DynamicImage::ImageLumaA8(_) => 2,
                image::DynamicImage::ImageRgb8(_) => 3,
                image::DynamicImage::ImageRgba8(_) => 4,
            };
            let (width, height) = image.dimensions();
            Ok(Some(ResourceData::D2(ResourceData2D {
                bytes: image.raw_pixels(),
                width: width,
                height: height,
                channels: channel_count,
                time: 0.0,
                xoffset: 0,
                yoffset: 0,
                subwidth: width,
                subheight: height,
                kind: ResourceDataKind::U8,
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
                let channel_count = match image {
                    image::DynamicImage::ImageLuma8(_) => 1,
                    image::DynamicImage::ImageLumaA8(_) => 2,
                    image::DynamicImage::ImageRgb8(_) => 3,
                    image::DynamicImage::ImageRgba8(_) => 4,
                };
                let (width, height) = image.dimensions();
                let resource = ResourceData2D {
                    bytes: image.raw_pixels(),
                    width: width,
                    height: height,
                    channels: channel_count,
                    time: 0.0,
                    xoffset: 0,
                    yoffset: 0,
                    subwidth: width,
                    subheight: height,
                    kind: ResourceDataKind::U8,
                };
                cubemap.push((*face, resource));
            }
            Ok(Some(ResourceData::Cube(cubemap)))
        }
        ResourceConfig::Texture3D(config) => {
            let f = std::fs::File::open(&config.texture_3d)
                .map_err(|err| Error::io(&config.texture_3d, err))?;
            let mut reader = std::io::BufReader::new(f);
            let mut bytes = Vec::new();
            let read = reader
                .read_to_end(&mut bytes)
                .map_err(|err| Error::io(&config.texture_3d, err))?;
            let w = config.resolution[0];
            let h = config.resolution[1];
            let d = config.resolution[2];
            assert!(read as u32 == w * h * d * config.components);
            Ok(Some(ResourceData::D3(ResourceData3D {
                bytes: bytes,
                width: w,
                height: h,
                depth: d,
                channels: config.components,
                time: 0.0,
                kind: ResourceDataKind::U8,
            })))
        }
        ResourceConfig::Video(_) => Ok(None),
        ResourceConfig::WebCam(_) => Ok(None),
        ResourceConfig::Audio(_) => Ok(None),
        ResourceConfig::Microphone(_) => Ok(None),
        ResourceConfig::Keyboard(_) => Ok(None),
        ResourceConfig::GstAppSinkPipeline(_) => Ok(None),
    }
}
