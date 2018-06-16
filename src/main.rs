extern crate byte_slice_cast;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate failure;
extern crate gleam;
#[macro_use]
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_audio as gst_audio;
extern crate gstreamer_video as gst_video;
#[macro_use]
extern crate log;
extern crate chrono;
extern crate env_logger;
extern crate image;
extern crate notify;
extern crate regex;
extern crate sdl2;
#[macro_use]
extern crate serde_derive;
extern crate toml;
#[macro_use]
extern crate lazy_static;

mod audio;
mod config;
mod effect;
mod error;
mod file_stream;
mod gl;
mod grimoire;
mod keyboard;
mod mouse;
mod platform;
mod resource;
mod stream;
mod video;

use std::env;
use std::process;
use std::result;
use std::time::{Duration, Instant};

use clap::{App, Arg};
use error::Error;
use grimoire::Grimoire;
use platform::Platform;
use sdl2::video::GLProfile;

/// Our type alias for handling errors throughout grimoire
type Result<T> = result::Result<T, failure::Error>;

fn main() {
    if let Err(err) = try_main() {
        // Print the error, including all of its underlying causes.
        error!("{}", pretty_error(&err));

        // If we get a non-empty backtrace (e.g., RUST_BACKTRACE=1 is set),
        // then show it.
        let backtrace = err.backtrace().to_string();
        if !backtrace.trim().is_empty() {
            eprintln!("{}", backtrace);
        }
        process::exit(1);
    }
}

fn try_main() -> Result<()> {
    env_logger::init();
    {
        let args: Vec<String> = env::args().collect();
        info!("{:?}", args);
    }
    let matches = App::new("grimoire")
        .version(crate_version!())
        .author(crate_authors!())
        .about("Run GLSL shader applications")
        .arg(
            Arg::with_name("shader")
                .help("path to the GLSL shader")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::with_name("width")
                .help("window pixel width")
                .takes_value(true)
                .default_value("768")
                .long("width")
                .requires("height"),
        )
        .arg(
            Arg::with_name("height")
                .help("window pixel height")
                .takes_value(true)
                .default_value("432")
                .long("height")
                .requires("width"),
        )
        .arg(
            Arg::with_name("gl")
                .help("opengl version")
                .takes_value(true)
                .possible_values(&[
                    "330", "400", "410", "420", "430", "440", "450", "460", "es2", "es3",
                ])
                .default_value("330")
                .long("gl"),
        )
        .get_matches();
    let width_str = matches.value_of("width").unwrap();
    let height_str = matches.value_of("height").unwrap();
    let effect_path = matches.value_of("shader").unwrap();
    let gl_str = matches.value_of("gl").unwrap();
    let width = width_str
        .parse::<u32>()
        .expect("Expected width command-line argument to be u32");
    let height = height_str
        .parse::<u32>()
        .expect("Expected height command-line argument to be u32");
    let (gl_major, gl_minor, gl_profile, glsl_version) = match gl_str {
        "330" => (3, 3, GLProfile::Core, "#version 330"),
        "400" => (4, 0, GLProfile::Core, "#version 400"),
        "410" => (4, 1, GLProfile::Core, "#version 410"),
        "420" => (4, 2, GLProfile::Core, "#version 420"),
        "430" => (4, 3, GLProfile::Core, "#version 430"),
        "440" => (4, 4, GLProfile::Core, "#version 440"),
        "450" => (4, 5, GLProfile::Core, "#version 450"),
        "460" => (4, 6, GLProfile::Core, "#version 460"),
        "es2" => (2, 0, GLProfile::GLES, "#version 100"),
        "es3" => (3, 0, GLProfile::GLES, "#version 300"),
        _ => unreachable!(),
    };

    let sdl_context = sdl2::init().map_err(Error::sdl2)?;
    let video_subsystem = sdl_context.video().map_err(Error::sdl2)?;
    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_version(gl_major, gl_minor);
    gl_attr.set_context_profile(gl_profile);
    // TODO(jshrake): These should be config/cli driven
    gl_attr.set_depth_size(24);
    gl_attr.set_framebuffer_srgb_compatible(true);
    gl_attr.set_multisample_buffers(1);
    gl_attr.set_multisample_samples(4);

    let window = video_subsystem
        .window("grimoire", width, height)
        .opengl()
        .resizable()
        .build()?;

    let _ctx = window.gl_create_context().map_err(Error::sdl2)?;
    debug_assert_eq!(gl_attr.context_profile(), gl_profile);
    debug_assert_eq!(gl_attr.context_version(), (gl_major, gl_minor));
    let gl = unsafe {
        gl::GlesFns::load_with(|addr| video_subsystem.gl_get_proc_address(addr) as *const _)
    };

    // If adaptive vsync is available, enable it, else just use vsync
    if !video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::LateSwapTearing) {
        video_subsystem.gl_set_swap_interval(sdl2::video::SwapInterval::VSync);
    }

    let mut event_pump = sdl_context.event_pump().map_err(Error::sdl2)?;
    gst::init()?;

    let mut absolute_effect_path = env::current_dir().expect("env::curent_dir() failed");
    absolute_effect_path.push(effect_path);
    let effect_path = absolute_effect_path.canonicalize().map_err(|err| {
        format_err!(
            "[PLATFORM] Error loading shader file {:?}: {}",
            absolute_effect_path,
            err
        )
    })?;
    let cwd = effect_path
        .parent()
        .expect("Expected shader file to have parent directory");
    env::set_current_dir(&cwd).expect(&format!("env::set_current_dir({:?}) failed", cwd));

    // Log Welcome Message + GL information
    info!(
        "Requested GL profile: {:?}, got {:?}",
        gl_profile,
        gl_attr.context_profile()
    );
    info!(
        "Requested GL version: {:?}, got {:?}",
        (gl_major, gl_minor),
        gl_attr.context_version()
    );
    {
        let vendor = gl.get_string(gl::VENDOR);
        let renderer = gl.get_string(gl::RENDERER);
        let version = gl.get_string(gl::VERSION);
        let shading_lang_version = gl.get_string(gl::SHADING_LANGUAGE_VERSION);
        let extension_count = gl.get_integer_v(gl::NUM_EXTENSIONS);
        let extensions: Vec<String> = (0..extension_count)
            .map(|i| gl.get_string_i(gl::EXTENSIONS, i as u32))
            .collect();
        info!("GL VENDOR:    {}", vendor);
        info!("GL RENDERER:  {}", renderer);
        info!("GL VERSION:   {}", version);
        info!("GLSL VERSION: {}", shading_lang_version);
        debug!("EXTENSIONS: {:?}", extensions);
    }
    let mut platform = Platform {
        events: &mut event_pump,
        gl: gl.clone(),
        window_resolution: window.size(),
        time_delta: Duration::from_secs(0),
    };
    let shader_header = include_str!("header.glsl");
    let shader_footer = include_str!("footer.glsl");
    let mut app = Grimoire::new(
        effect_path.as_path(),
        glsl_version.to_string(),
        shader_header.to_string(),
        shader_footer.to_string(),
    )?;
    app.play()?;
    let mut frame_count = 0;
    let mut total_elapsed: Duration = Default::default();
    let frame_window = 600;
    'running: loop {
        let now = Instant::now();
        match app.tick(&mut platform) {
            Err(err) => error!("{}", pretty_error(&failure::Error::from(err))),
            Ok(should_quit) => if should_quit {
                break 'running;
            },
        }
        window.gl_swap_window();
        platform.time_delta = now.elapsed();
        platform.window_resolution = window.size();
        frame_count += 1;
        total_elapsed += platform.time_delta;
        if frame_count > frame_window {
            fn duration_to_float_secs(duration: Duration) -> f32 {
                duration.as_secs() as f32 + duration.subsec_nanos() as f32 * 1e-9
            }
            debug!(
                "[PLATFORM] Average frame time over last {} frames: {} seconds",
                frame_window,
                duration_to_float_secs(total_elapsed) / frame_window as f32
            );
            frame_count = Default::default();
            total_elapsed = Default::default();
        }
    }
    Ok(())
}

/// Return a prettily formatted error, including its entire causal chain.
fn pretty_error(err: &failure::Error) -> String {
    let mut pretty = String::new();
    pretty.push_str(&err.to_string());
    let mut prev = err.cause();
    while let Some(next) = prev.cause() {
        pretty.push_str("\n");
        pretty.push_str(&next.to_string());
        prev = next;
    }
    pretty
}
