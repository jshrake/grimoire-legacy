use std::error::Error as StdError;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::Mutex;

use byte_slice_cast::*;
use config::TextureFormat;
use error::{Error, Result};
use gst;
use gst::prelude::*;
use gst_app;
use gst_video;
use resource::{ResourceData, ResourceData2D};
use stream::Stream;

#[derive(Debug)]
pub struct Video {
    pipeline: gst::Element,
    receiver: Receiver<ResourceData2D>,
}

impl Video {
    pub fn new_video(uri: &str) -> Result<Self> {
        let playbin = gst::ElementFactory::make("playbin", None)
            .ok_or(Error::gstreamer("missing playbin element"))?;
        let sink = gst::ElementFactory::make("appsink", None)
            .ok_or(Error::gstreamer("missing appsink element"))?;
        playbin
            .set_property("uri", &uri.to_string())
            .map_err(|err| {
                Error::gstreamer(format!(
                    "error setting uri property of playbin element: {}",
                    err
                ))
            })?;
        playbin.set_property("video-sink", &sink).map_err(|err| {
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
            &[("format", &gst_video::VideoFormat::Rgb.to_string())],
        ));
        let data_pipe = data_pipe_from_appsink(appsink)?;
        Ok(Self {
            pipeline: playbin,
            receiver: data_pipe,
        })
    }

    pub fn new_webcam() -> Result<Self> {
        let pipeline = "autovideosrc device=/dev/videoX ! video/x-raw,framerate=(fraction)30/1,width=1280,height=720 ! videoconvert ! video/x-raw,format=RGB ! appsink name=appsink";
        let pipeline = gst::parse_launch(&pipeline).map_err(|e| Error::gstreamer(e.to_string()))?;
        let sink = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .get_by_name("appsink")
            .ok_or(Error::bug(
                "[VIDEO] Pipelink does not contain element with name 'appsink'",
            ))?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| Error::bug("[VIDEO] Sink element is expected to be an appsink"))?;
        let data_pipe = data_pipe_from_appsink(appsink)?;
        Ok(Self {
            pipeline: pipeline,
            receiver: data_pipe,
        })
    }

    pub fn new_appsink_pipeline(pipeline: &str) -> Result<Self> {
        let pipeline = gst::parse_launch(&pipeline).map_err(|e| Error::gstreamer(e.to_string()))?;
        let sink = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .get_by_name("appsink")
            .ok_or(Error::gstreamer(
                "Pipelink must have an appsink element named 'appsink'",
            ))?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| Error::gstreamer("Sink element is expected to be an appsink"))?;
        let data_pipe = data_pipe_from_appsink(appsink)?;
        Ok(Self {
            pipeline: pipeline,
            receiver: data_pipe,
        })
    }
}

impl Drop for Video {
    fn drop(&mut self) {
        self.pipeline
            .set_state(gst::State::Null)
            .into_result()
            .unwrap();
    }
}

impl Stream for Video {
    fn play(&mut self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.pipeline
            .set_state(gst::State::Paused)
            .into_result()
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn restart(&mut self) -> Result<()> {
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                0 * gst::SECOND,
            )
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let bus = self
            .pipeline
            .get_bus()
            .ok_or(Error::bug("[GSTREAMER] Video pipeline with no bus"))?;
        while let Some(msg) = bus.timed_pop(gst::ClockTime::from_seconds(0)) {
            use gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    // Default behavior is to loop
                    self.restart()?;
                }
                MessageView::Error(err) => {
                    let src = err
                        .get_src()
                        .map(|s| s.get_path_string())
                        .unwrap_or_else(|| String::from("None"));
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
                Some(q.get_result())
            } else {
                None
            }
        }.and_then(|pos| pos.try_into_time().ok())
            .unwrap_or(gst::ClockTime::from_seconds(0));
        let playback_position: f32 =
            (playback_position.nanoseconds().unwrap_or(0) as f64 / 1_000_000_000u64 as f64) as f32;
        match self.receiver.try_recv() {
            Ok(mut resource) => {
                resource.time = playback_position;
                match dest.send(ResourceData::D2(resource)) {
                    Err(_) => (),
                    _ => (),
                }
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => {
                self.pipeline
                    .set_state(gst::State::Null)
                    .into_result()
                    .map_err(|e| Error::gstreamer(e.to_string()))?;
            }
        };
        Ok(())
    }
}

fn data_pipe_from_appsink(appsink: gst_app::AppSink) -> Result<Receiver<ResourceData2D>> {
    let (tx, rx) = channel();
    let tx_mutex = Mutex::from(tx);
    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::new()
            .new_sample(move |appsink| {
                let sample = match appsink.pull_sample() {
                    None => return gst::FlowReturn::Eos,
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
                    return gst::FlowReturn::Error;
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
                        return gst::FlowReturn::Error;
                    };

                let buffer = if let Some(buffer) = sample.get_buffer() {
                    buffer
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to get buffer from appsink")
                    );

                    return gst::FlowReturn::Error;
                };

                let map = if let Some(map) = buffer.map_readable() {
                    map
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to map buffer readable")
                    );

                    return gst::FlowReturn::Error;
                };

                let samples = if let Ok(samples) = map.as_slice().as_slice_of::<u8>() {
                    samples
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/VIDEO] Failed to interpret buffer as u8")
                    );

                    return gst::FlowReturn::Error;
                };
                let format = match video_info.format() {
                    gst_video::VideoFormat::Gray16Be => TextureFormat::RF16,
                    gst_video::VideoFormat::Gray16Le => TextureFormat::RF16,
                    _ => TextureFormat::RGBU8,
                };
                let bytes = Vec::from(samples);
                let resource = ResourceData2D {
                    bytes: bytes,
                    width: video_info.width(),
                    height: video_info.height(),
                    format: format,
                    subwidth: video_info.width(),
                    subheight: video_info.height(),
                    xoffset: 0,
                    yoffset: 0,
                    time: 0.0,
                };
                let tx = tx_mutex.lock().unwrap();
                tx.send(resource).unwrap();
                gst::FlowReturn::Ok
            })
            .build(),
    );
    Ok(rx)
}
