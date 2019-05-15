use crate::config::TextureFormat;
use crate::error::{Error, Result};
use crate::gst;
use crate::gst::prelude::*;
use crate::gst_app;
use crate::gst_audio;
use crate::resource::{ResourceData, ResourceData2D};
use crate::stream::Stream;
use byte_slice_cast::*;
use std;
use std::error::Error as StdError;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;

#[derive(Debug)]
pub struct Audio {
    pipeline: gst::Element,
    receiver: Receiver<ResourceData2D>,
    bands: usize,
    previous_fft: Vec<u8>,
}

// I believe that shadertoy.com uses the dfault values from the AnalyserNode. See:
// https://developer.mozilla.org/en-US/docs/Web/API/AnalyserNode/minDecibels
// https://developer.mozilla.org/en-US/docs/Web/API/AnalyserNode/maxDecibels
// https://developer.mozilla.org/en-US/docs/Web/API/AnalyserNode/smoothingTimeConstant
static MIN_DB: f32 = -100.0;
static MAX_DB: f32 = -30.0;
static SMOOTH: f32 = 0.8;

impl Audio {
    pub fn new_audio(uri: &str, bands: usize) -> Result<Self> {
        let pipeline = format!(
                "uridecodebin uri={uri} ! tee name=t ! \
                queue ! audioconvert ! audioresample ! audio/x-raw,format=U8,channels=1 ! appsink name=appsink async=false sync=true t. ! \
                queue ! audioconvert ! audioresample ! audio/x-raw,channels=1  ! spectrum bands={bands} threshold={thresh} interval=16000000 \
                    post-messages=true message-magnitude=true ! fakesink async=false sync=true t. ! \
                queue ! audioconvert ! audioresample ! autoaudiosink async=true
                ", uri=uri, bands=2*bands, thresh=MIN_DB);
        Audio::from_pipeline(&pipeline, bands)
    }

    pub fn new_microphone(bands: usize) -> Result<Self> {
        let pipeline = format!(
                "autoaudiosrc ! tee name=t ! \
                queue ! audioconvert ! audioresample ! audio/x-raw,format=U8,channels=1 ! appsink name=appsink t. ! \
                queue ! audioconvert ! audioresample ! audio/x-raw,channels=1 ! spectrum bands={bands} threshold={thresh} interval=16000000 \
                    post-messages=true message-magnitude=true ! fakesink",
                bands=2*bands, thresh=MIN_DB);
        Audio::from_pipeline(&pipeline, bands)
    }

    pub fn from_pipeline(pipeline: &str, bands: usize) -> Result<Self> {
        let pipeline = gst::parse_launch(&pipeline).map_err(|e| Error::gstreamer(e.to_string()))?;
        pipeline
            .set_state(gst::State::Ready)
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        let sink = pipeline
            .clone()
            .dynamic_cast::<gst::Bin>()
            .unwrap()
            .get_by_name("appsink")
            .ok_or_else(|| {
                Error::bug("[GRIMOIRE/AUDIO] Pipelink does not contain element with name 'appsink'")
            })?;
        let appsink = sink
            .clone()
            .dynamic_cast::<gst_app::AppSink>()
            .map_err(|_| Error::bug("[GRIMOIRE/AUDIO] Expected sink element to be an appsink"))?;
        let receiver = gst_sample_receiver_from_appsink(&appsink, bands)?;
        Ok(Self {
            pipeline,
            bands,
            receiver,
            previous_fft: vec![0; bands],
        })
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        self.pipeline.set_state(gst::State::Null).unwrap();
    }
}

impl Stream for Audio {
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
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::from_seconds(0),
            )
            .map_err(|e| Error::gstreamer(e.to_string()))?;
        Ok(())
    }

    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let bus = self
            .pipeline
            .get_bus()
            .ok_or_else(|| Error::gstreamer("pipeline contains no bus"))?;
        while let Some(msg) = bus.timed_pop(gst::ClockTime::from_seconds(0)) {
            use crate::gst::MessageView;
            match msg.view() {
                MessageView::Eos(..) => {
                    self.restart()?;
                }
                MessageView::Error(err) => {
                    self.pipeline
                        .set_state(gst::State::Null)
                        .map_err(|e| Error::gstreamer(e.to_string()))?;
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
                MessageView::Element(element) => {
                    if let Some(structure) = element.get_structure() {
                        if structure.get_name() == "spectrum" {
                            let _endtime = structure
                                .get::<gst::ClockTime>("endtime")
                                .unwrap_or_else(gst::ClockTime::none);
                            let magnitude = structure.get_value("magnitude").unwrap();
                            let magnitude = magnitude.get::<gst::List>().unwrap();
                            // We expect the magnitude length to be 2x the # of bands
                            assert!(2 * self.bands == magnitude.as_slice().len());
                            // normalize the magnitude to [0.0, 1.0]
                            let magnitude: Vec<f32> = magnitude
                                .as_slice()
                                .iter()
                                .take(self.bands)
                                .map(|v| {
                                    v.get::<f32>()
                                        .expect("Expect spectrum gst::List to contain f32")
                                })
                                .collect();
                            let scale = 255.0 / (MIN_DB - MAX_DB);
                            let magnitude: Vec<u8> = magnitude
                                .into_iter()
                                .map(|f| f32::min(f, MAX_DB))
                                .map(|f| 255 - ((f - MAX_DB) * scale) as u8)
                                .collect();
                            let magnitude_len = magnitude.len();
                            let smoothed_magnitude: Vec<_> = magnitude
                                .iter()
                                .enumerate()
                                .map(|(i, m)| {
                                    ((1.0 - SMOOTH) * (*m as f32)
                                        + SMOOTH * (self.previous_fft[i] as f32))
                                        as u8
                                })
                                .collect();
                            self.previous_fft = smoothed_magnitude.clone();
                            let resource = ResourceData::D2(ResourceData2D {
                                bytes: smoothed_magnitude,
                                width: self.bands as u32,
                                height: 2,
                                format: TextureFormat::RU8,
                                xoffset: 0,
                                yoffset: 0,
                                subwidth: magnitude_len as u32, // Only upload one row of data
                                subheight: 1,                   // Upload to the second row
                                time: -1.0,                     // endtime
                            });
                            dest.send(resource).unwrap();
                        }
                    }
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
        }
        .and_then(|pos| pos.try_into_time().ok())
        .unwrap_or_else(|| gst::ClockTime::from_seconds(0));
        let playback_position: f32 =
            (playback_position.nanoseconds().unwrap_or(0) as f64 / 1_000_000_000u64 as f64) as f32;
        let mut data = None;
        while let Some(resource) = self.receiver.try_iter().next() {
            data = Some(resource);
        }
        if let Some(mut data) = data {
            // Update the time stamp to match the playback position
            data.time = playback_position;
            match dest.send(ResourceData::D2(data)) {
                _ => (),
            }
        }
        Ok(())
    }
}

fn gst_sample_receiver_from_appsink(
    appsink: &gst_app::AppSink,
    bands: usize,
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
                        ("[GRIMOIRE/AUDIO] Failed to get caps from appsink sample")
                    );
                    return Err(gst::FlowError::Error);
                };

                let _info = if let Some(info) = gst_audio::AudioInfo::from_caps(&sample_caps) {
                    info
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/AUDIO] Failed to build AudioInfo from caps")
                    );
                    return Err(gst::FlowError::Error);
                };

                let buffer = if let Some(buffer) = sample.get_buffer() {
                    buffer
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/AUDIO] Failed to get buffer from appsink")
                    );
                    return Err(gst::FlowError::Error);
                };

                let map = if let Some(map) = buffer.map_readable() {
                    map
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/AUDIO] Failed to map buffer readable")
                    );
                    return Err(gst::FlowError::Error);
                };

                let samples = if let Ok(samples) = map.as_slice().as_slice_of::<u8>() {
                    samples
                } else {
                    gst_element_error!(
                        appsink,
                        gst::ResourceError::Failed,
                        ("[GRIMOIRE/AUDIO] Failed to interpret buffer as u8")
                    );
                    return Err(gst::FlowError::Error);
                };
                let bytes = Vec::from(samples);
                let bytes: Vec<u8> = bytes.into_iter().take(bands).collect();
                let tx = tx_mutex.lock().unwrap();
                let bytes_len = bytes.len();
                let resource = ResourceData2D {
                    bytes,
                    width: bands as u32,
                    height: 2,
                    format: TextureFormat::RU8,
                    subwidth: bytes_len as u32,
                    subheight: 1,
                    xoffset: 0,
                    yoffset: 1,
                    time: 0.0,
                };
                tx.send(resource).unwrap();
                Ok(gst::FlowSuccess::Ok)
            })
            .build(),
    );
    Ok(rx)
}
