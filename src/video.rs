use crate::config::TextureFormat;
use crate::error::{Error, Result};
use crate::gst;
use crate::gst::prelude::*;
use crate::gst_app;
use crate::gst_video;
use crate::resource::{ResourceData, ResourceData2D};
use crate::stream::Stream;
use byte_slice_cast::*;
use std::error::Error as StdError;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;

#[derive(Debug)]
pub struct Video {
    pipeline: gst::Element,
    receiver: Receiver<ResourceData2D>,
}

impl Video {
    pub fn new_video(uri: &str) -> Result<Self> {
        let pipeline = gst::ElementFactory::make("playbin", None)
            .ok_or_else(|| Error::gstreamer("missing playbin element"))?;
        let sink = gst::ElementFactory::make("appsink", None)
            .ok_or_else(|| Error::gstreamer("missing appsink element"))?;
        pipeline
            .set_property("uri", &uri.to_string())
            .map_err(|err| {
                Error::gstreamer(format!(
                    "error setting uri property of playbin element: {}",
                    err
                ))
            })?;
        pipeline.set_property("video-sink", &sink).map_err(|err| {
            Error::gstreamer(format!(
                "error setting video-sink property of playbin element {}",
                err
            ))
        })?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .expect("Sink element is expected to be an appsink!");
        appsink.set_caps(&gst::Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &gst_video::VideoFormat::Rgb.to_string()),
                ("format", &gst_video::VideoFormat::Rgba.to_string()),
                ("format", &gst_video::VideoFormat::Bgr.to_string()),
                ("format", &gst_video::VideoFormat::Bgra.to_string()),
            ],
        ));
        let receiver = gst_sample_receiver_from_appsink(&appsink)?;
        Ok(Self { pipeline, receiver })
    }

    pub fn new_webcam() -> Result<Self> {
        let pipeline = "autovideosrc ! video/x-raw,format=RGB,format=RGBA,format=BGR,format=BGRA ! appsink name=appsink async=false sync=false";
        let pipeline = gst::parse_launch(&pipeline).map_err(|e| Error::gstreamer(e.to_string()))?;
        let sink = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .get_by_name("appsink")
            .ok_or_else(|| {
                Error::bug("[VIDEO] Pipelink does not contain element with name 'appsink'")
            })?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| Error::bug("[VIDEO] Sink element is expected to be an appsink"))?;
        let receiver = gst_sample_receiver_from_appsink(&appsink)?;
        Ok(Self { pipeline, receiver })
    }

    pub fn new_appsink_pipeline(pipeline: &str) -> Result<Self> {
        let pipeline = gst::parse_launch(&pipeline).map_err(|e| Error::gstreamer(e.to_string()))?;
        let sink = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .get_by_name("appsink")
            .ok_or_else(|| {
                Error::gstreamer("Pipelink must have an appsink element named 'appsink'")
            })?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| Error::gstreamer("Sink element is expected to be an appsink"))?;
        let receiver = gst_sample_receiver_from_appsink(&appsink)?;
        Ok(Self { pipeline, receiver })
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        self.pipeline.set_state(gst::State::Null).unwrap();
    }
}

impl Stream for Video {
    fn play(&mut self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Paused)
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn restart(&mut self) -> Result<()> {
        // Swallow any errors
        // You can't seek some pipelines, like the webcam pipeline.
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::from_seconds(0),
            )
            .ok();
        Ok(())
    }

    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let bus = self
            .pipeline
            .get_bus()
            .ok_or_else(|| Error::bug("[GSTREAMER] Video pipeline with no bus"))?;
        while let Some(msg) = bus.timed_pop(gst::ClockTime::from_seconds(0)) {
            use crate::gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    // Default behavior is to loop
                    self.restart()?;
                }
                MessageView::Error(err) => {
                    let src = err
                        .get_src()
                        .map(|s| s.get_path_string())
                        .unwrap_or_else(|| gst::glib::GString::from("None"));
                    let error: String = err.get_error().description().into();
                    let debug = err.get_debug();
                    return Err(Error::gstreamer(format!(
                        "bus error: {} from source element {}. debug {:?}",
                        error, src, debug
                    )));
                }
                _ => {}
            }
        }
        let playback_position = {
            let mut q = gst::Query::new_position(gst::Format::Time);
            if self.pipeline.query(&mut q) {
                q.get_result()
                    .try_into_time()
                    .unwrap_or_else(|_| gst::ClockTime::from_seconds(0))
            } else {
                gst::ClockTime::from_seconds(0)
            }
        };
        let playback_position: f32 =
            (playback_position.nanoseconds().unwrap_or(0) as f64 / 1_000_000_000u64 as f64) as f32;
        match self.receiver.try_recv() {
            Ok(mut resource) => {
                resource.time = playback_position;
                if dest.send(ResourceData::D2(resource)).is_err() {
                    info!("video::stream_to: error sending D2 resource. Continuing...");
                }
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                self.pipeline
                    .set_state(gst::State::Null)
                    .map_err(|e| Error::gstreamer(e.to_string()))?;
            }
        };
        Ok(())
    }
}

fn gst_sample_receiver_from_appsink(
    appsink: &gst_app::AppSink,
) -> Result<Receiver<ResourceData2D>> {
    let (tx, rx) = channel();
    let tx_mutex = Mutex::from(tx);
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::new()
            .new_sample(move |appsink| {
                let sample = match appsink.pull_sample() {
                    None => return Err(gst::FlowError::Eos),
                    Some(sample) => sample,
                };

                let sample_caps = if let Some(sample_caps) = sample.get_caps() {
                    sample_caps
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to get caps from appsink sample")
                    );
                    return Err(gst::FlowError::Error);
                };

                let video_info =
                    if let Some(video_info) = gst_video::VideoInfo::from_caps(&sample_caps) {
                        video_info
                    } else {
                        gst_element_error!(
                            appsink,
                            gst::ResourceError::Failed,
                            ("[GRIMOIRE/VIDEO] Failed to build VideoInfo from caps")
                        );
                        return Err(gst::FlowError::Error);
                    };

                let buffer = if let Some(buffer) = sample.get_buffer() {
                    buffer
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to get buffer from appsink")
                    );

                    return Err(gst::FlowError::Error);
                };

                let map = if let Some(map) = buffer.map_readable() {
                    map
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to map buffer readable")
                    );

                    return Err(gst::FlowError::Error);
                };

                let samples = if let Ok(samples) = map.as_slice().as_slice_of::<u8>() {
                    samples
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to interpret buffer as u8")
                    );

                    return Err(gst::FlowError::Error);
                };
                let format = match video_info.format() {
                    gst_video::VideoFormat::Rgb => TextureFormat::RGBU8,
                    gst_video::VideoFormat::Rgba => TextureFormat::RGBAU8,
                    gst_video::VideoFormat::Bgr => TextureFormat::BGRU8,
                    gst_video::VideoFormat::Bgra => TextureFormat::BGRAU8,
                    gst_video::VideoFormat::Gray16Le => TextureFormat::RF16,
                    unsupported_format => {
                        gst_element_error!(
                            appsink,
                            gst::ResourceError::Failed,
                            (
                                "[GRIMOIRE/VIDEO] Unsupported video format {:?}",
                                unsupported_format
                            )
                        );
                        return Err(gst::FlowError::Error);
                    }
                };
                let bytes = Vec::from(samples);
                let resource = ResourceData2D {
                    bytes,
                    format,
                    width: video_info.width(),
                    height: video_info.height(),
                    subwidth: video_info.width(),
                    subheight: video_info.height(),
                    xoffset: 0,
                    yoffset: 0,
                    time: 0.0,
                };
                let tx = tx_mutex.lock().unwrap();
                tx.send(resource).unwrap();
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    Ok(rx)
}
