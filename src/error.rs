use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::result;
use std::string;

use failure::{Backtrace, Context, Fail};
use image;
use notify;
use toml;

/// A type alias for handling errors throughout grimoire
pub type Result<T> = result::Result<T, Error>;

/// An error that can occur while running grimoire
/// Some errors are actionable by the user, while others indicate a bug or a misconfigured system
#[derive(Debug)]
pub struct Error {
    ctx: Context<ErrorKind>,
}

impl Error {
    /// Return the kind of this error.
    #[allow(dead_code)]
    pub fn kind(&self) -> &ErrorKind {
        self.ctx.get_context()
    }

    pub(crate) fn notify(err: notify::Error) -> Error {
        Error::from(ErrorKind::Notify(err.to_string()))
    }

    pub(crate) fn watch_path<P: AsRef<Path>>(path: P, err: notify::Error) -> Error {
        Error::from(ErrorKind::WatchPath(
            path.as_ref().to_path_buf(),
            err.to_string(),
        ))
    }

    pub(crate) fn image<P: AsRef<Path>>(path: P, err: image::ImageError) -> Error {
        Error::from(ErrorKind::Image(
            path.as_ref().to_path_buf(),
            err.to_string(),
        ))
    }

    pub(crate) fn io<P: AsRef<Path>>(path: P, err: io::Error) -> Error {
        Error::from(ErrorKind::Io(path.as_ref().to_path_buf(), err.to_string()))
    }

    pub(crate) fn toml(err: toml::de::Error) -> Error {
        Error::from(ErrorKind::Toml(err.to_string()))
    }

    pub(crate) fn from_utf8<P: AsRef<Path>>(path: P, err: string::FromUtf8Error) -> Error {
        Error::from(ErrorKind::FromUtf8(
            path.as_ref().to_path_buf(),
            err.to_string(),
        ))
    }

    pub(crate) fn glsl_vertex<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::GlslVertex(msg.as_ref().to_string()))
    }

    pub(crate) fn glsl_fragment<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::GlslFragment(msg.as_ref().to_string()))
    }

    pub(crate) fn glsl_program<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::GlslProgram(msg.as_ref().to_string()))
    }

    pub(crate) fn sdl2<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::SDL2(msg.as_ref().to_string()))
    }

    pub(crate) fn gstreamer<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::Gstreamer(msg.as_ref().to_string()))
    }

    pub(crate) fn resource_not_found<T: AsRef<str>>(name: T, channel: T, names: Vec<T>) -> Error {
        Error::from(ErrorKind::ResourceNotFound(
            name.as_ref().to_string(),
            channel.as_ref().to_string(),
            names.iter().map(|s| s.as_ref().to_string()).collect(),
        ))
    }

    pub(crate) fn bug<T: AsRef<str>>(msg: T) -> Error {
        Error::from(ErrorKind::Bug(msg.as_ref().to_string()))
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.ctx.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.ctx.backtrace()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.ctx.fmt(f)
    }
}

/// The specific kind of error that can occur.
#[derive(Clone, Debug)]
pub enum ErrorKind {
    /// An error loading a resource occurred.
    ///
    /// The data provided is the resource name from the configuration.
    BadResourceConfig(String),
    /// An error loading an image occurred.
    Image(PathBuf, String),
    /// An unexpected I/O error occurred.
    Io(PathBuf, String),
    /// An error watching a path occurred.
    WatchPath(PathBuf, String),
    /// An unexpected Utf8 error occured.
    FromUtf8(PathBuf, String),
    /// An error occurred while parsing the TOML configuration
    Toml(String),
    /// An error compiling a GLSL vertex shader.
    ///
    /// The data provided is the GLSL error.
    GlslVertex(String),
    /// An error compiling a GLSL fragment shader.
    ///
    /// The data provided is the GLSL error.
    GlslFragment(String),
    /// An error linking a GLSL program.
    ///
    /// The data provided is the GLSL error.
    GlslProgram(String),
    /// A gstreamer error occurred.
    Gstreamer(String),
    /// An error finding a resource.
    ///
    /// The data provided is the unrecognized name, and the channel name attempting to use it.
    ResourceNotFound(String, String, Vec<String>),
    /// An error during the pass construction.
    ///
    /// The data provided is the pass index
    GLPass(usize),
    /// An error with notify occurred.
    Notify(String),
    /// An error with SDL2 occurred.
    SDL2(String),
    /// An unexpected error occurred. Generally, these errors correspond
    /// to bugs in grimoire.
    Bug(String),
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ErrorKind::Image(ref path, ref err) => {
                write!(f, "Error loading image at {:?}: {}", path, err)
            }
            ErrorKind::Io(ref path, ref err) => {
                write!(f, "Error performing I/O operation on {:?}: {}", path, err)
            }
            ErrorKind::WatchPath(ref path, ref err) => {
                write!(f, "Error watching path {:?}: {}", path, err)
            }
            ErrorKind::BadResourceConfig(ref name) => {
                write!(f, "Error loading resource \"{}\"", name)
            }
            ErrorKind::Notify(ref err) => write!(f, "{}", err),
            ErrorKind::FromUtf8(ref path, ref err) => write!(
                f,
                "Error calling String::from_utf8 on bytes from file {:?}: {}",
                path, err
            ),
            ErrorKind::GlslVertex(ref err) => {
                write!(f, "[GLSL] Error compiling vertex shader: {}", err)
            }
            ErrorKind::GlslFragment(ref err) => {
                write!(f, "[GLSL] Error compiling fragment shader: {}", err)
            }
            ErrorKind::GlslProgram(ref err) => write!(f, "[GLSL] Error linking program: {}", err),
            ErrorKind::Toml(ref err) => write!(f, "[TOML] Error parsing configuration: {}", err),
            ErrorKind::GLPass(ref index) => write!(f, "Error building [[pass]] {}", index),
            ErrorKind::ResourceNotFound(ref name, ref channel, ref available_resources) => write!(
                f,
                "Bad channel map {} = \"{}\": resource \"{}\" not found. Valid resource names: {:?}",
                channel, name, name, available_resources
            ),
            ErrorKind::SDL2(ref err) => write!(f, "[SDL2]: {}", err),
            ErrorKind::Gstreamer(ref err) => write!(
                f,
                "[GSTREAMER] {:?} (Run with GST_DEBUG=3 for more information)",
                err
            ),
            ErrorKind::Bug(ref msg) => {
                let report = "Please report this bug with a backtrace at \
                              https://github.com/jshrake/grimoire";
                write!(f, "[BUG] {}\n{}", msg, report)
            }
        }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Error {
        Error::from(Context::new(kind))
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(ctx: Context<ErrorKind>) -> Error {
        Error { ctx }
    }
}
