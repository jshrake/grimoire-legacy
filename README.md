# grimoire &emsp; [![BUILD-img]][BUILD-link] [![CRATES-img]][CRATES-link] [![MIT-img]][MIT-link] [![APACHE-img]][APACHE-link]

[BUILD-img]: https://travis-ci.org/jshrake/grimoire.svg?branch=master
[BUILD-link]: https://travis-ci.org/jshrake/grimoire
[CRATES-img]: https://img.shields.io/crates/v/grimoire.svg
[CRATES-link]: https://crates.io/crates/grimoire
[MIT-img]: http://img.shields.io/badge/license-MIT-blue.svg
[MIT-link]: https://github.com/jshrake/grimoire/blob/master/LICENSE-MIT
[APACHE-img]: https://img.shields.io/badge/License-Apache%202.0-blue.svg
[APACHE-link]: https://github.com/jshrake/grimoire/blob/master/LICENSE-APACHE

## What?

<a href="https://github.com/jshrake/grimoire-examples/blob/master/volume.glsl"><img src="https://thumbs.gfycat.com/CriminalEnergeticBird-size_restricted.gif" width="280" height="200" /></a> <a href="https://github.com/jshrake/grimoire-examples/blob/master/kinect2-raymarch.glsl"><img src="https://thumbs.gfycat.com/LikableJoyfulAsianelephant-size_restricted.gif" width="280" height="200" /></a> <a href="https://github.com/jshrake/grimoire/blob/master/examples/scene-0001.glsl"><img src="https://thumbs.gfycat.com/OffensiveEnragedGemsbok-size_restricted.gif" width="280" height="200" /></a>

grimoire is a cross-platform (Windows, MacOS, "Linux") live-coding tool for creating GLSL shader demos in the style of [shadertoy](https://www.shadertoy.com/) and [vertexshaderart](https://www.vertexshaderart.com). Users write a TOML configuration file that defines resources (image, video, audio, webcam, 3D texture, kinect data) and render passes (vertex shader, fragment shader, primitive type and count, blend state, depth state, uniform samplers). Your shaders, resources, and config file are watched for changes and are live updated at runtime. See the examples below to get started or read the [SPEC.md](./SPEC.md) for a detailed description of the configuration schema and runtime behavior.

**[grimoire is my personal prototyping tool](https://instagram.com/jlshrake) and is in the early stages of development. You may encounter bugs and features may change without notice. Do not expect support. With that out of the way, I think you will find grimoire an easy to use, robust, and powerful tool for prototyping shader effects. Feedback is welcome!**

### examples: shadertoy compatibility

The following shaders demonstrate compatibility with various shadertoy features. All content is copyright by the original author and licensed under the terms specified, or by the [default shadertoy license](https://www.shadertoy.com/terms). If you do not want your work included in this list, you can contact me and I will remove it immediately.

- [new](./examples/shadertoy-new/), [source](https://www.shadertoy.com/new): `cargo run -- ./examples/shadertoy-new`
- [debug](./examples/shadertoy-debug/), [source](https://www.shadertoy.com/view/llySRh): `cargo run -- ./examples/shadertoy-debug`
- [mouse](./examples/shadertoy-mouse/), [source](https://www.shadertoy.com/view/Mss3zH): `cargo run -- ./examples/shadertoy-mouse`
- [keyboard](./examples/shadertoy-keyboard-debug/), [source](https://www.shadertoy.com/view/4dGyDm): `cargo run -- ./examples/shadertoy-keyboard-debug`
- [time](./examples/shadertoy-time/), [source](https://www.shadertoy.com/view/lsXGz8): `cargo run -- ./examples/shadertoy-time`
- [fps](./examples/shadertoy-fps/), [source](https://www.shadertoy.com/view/lsKGWV): `cargo run -- ./examples/shadertoy-fps`
- [microphone](./examples/shadertoy-microphone/), [source](https://www.shadertoy.com/view/llSGDh): `cargo run -- ./examples/shadertoy-microphone`
- [sound](./examples/shadertoy-sound/), [source](https://www.shadertoy.com/view/Xds3Rr): `cargo run -- ./examples/shadertoy-sound`
- [multipass w/ feedback](./examples/shadertoy-deformation-feedback), [source](https://www.shadertoy.com/view/Xdd3DB): `cargo run -- ./examples/shadertoy-deformation-feedback`
- [video](./examples/video/): `cargo run -- ./examples/video`
- [webcam](./examples/webcam/): `cargo run -- ./examples/webcam`

## Install

You need to build and install grimoire from source using [rust](https://www.rust-lang.org/en-US/install.html) and install the required system dependencies:

- [SDL2](https://wiki.libsdl.org/Installation) for window and input handling
- [GStreamer](https://GStreamer.freedesktop.org/documentation/installing/index.html) for video, webcam, audio, microphone, and kinect2 inputs
- OpenGL 3.3+, but uses a subset of OpenGL accessible from GLES 3.0

grimoire currently builds against rust stable 1.33, 2018 edition.

### MacOS

```console
$ curl https://sh.rustup.rs -sSf | sh
$ brew install sdl2 gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav
```

If running on MacOS 10.14 (Mojave), be sure to manually copy [Info.plist](./Info.plist) to `target/debug` or `target/release` before running a demo that uses a webcam or microphone resource. The presence of this file allows MacOS to prompt for permission to access the camera and microphone.

If you encounter a build error similar to "Package libffi was not found in the pkg-config search path" you may need to issue something like this prior to build:

```console
$ export PKG_CONFIG_PATH=/usr/local/opt/libffi/lib/pkgconfig
```

### Linux

```console
$ curl https://sh.rustup.rs -sSf | sh
$ apt-get install libsdl2-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
```

### Windows

- Download and run [msys2](https://www.msys2.org/)
- Use the `x86_64-pc-windows-gnu` toolchain: `rustup default stable-x86_64-pc-windows-gnu`
- Install the required dependencies /w pacman

```console
$ pacman -S mingw-w64-x86_64-pkg-config mingw-w64-x86_64-SDL2 mingw-w64-x86_64-GStreamer mingw-w64-x86_64-gst-plugins-base mingw-w64-x86_64-gst-plugins-good mingw-w64-x86_64-gst-plugins-bad mingw-w64-x86_64-gst-plugins-ugly mingw-w64-x86_64-gst-libav
```
- Manually copy the SDL2.dll to the top-level grimoire source directory (the one containing Cargo.toml) before running

Note that you need to ensure that your `PATH` contains the mingw64/bin directory, and that your `PKG_CONFIG_PATH` lists the directory containing all the .pc files. Since I installed msys2 with scoop, my `.bash_profile` contains the following lines:

```
PATH="$PATH:/c/Users/jshrake/scoop/apps/msys2/current/mingw64/bin"
PKG_CONFIG_PATH="/c/Users/jshrake/scoop/apps/msys2/current/mingw64/lib/pkgconfig"
```

Breadcrumbs:
- https://github.com/sdroege/GStreamer-rs#windows

### Build

Rust uses [cargo](https://doc.rust-lang.org/cargo/) to manage package dependencies and build:

```console
$ cargo build
```

or with optimizations:

```console
$ cargo build --release
```

### Run

Display help:

```console
cargo run -- --help
```

To run the default new shadertoy shader:

```console
RUST_LOG=info cargo run -- ./examples/shadertoy-new/
```

grimoire will watch for saved changes to any file referenced by ./examples/shadertoy-new/grim.toml, including shaders and assets.

See [the log crate documentation](https://docs.rs/log/0.4.6/log/) for more logging levels.

### Playback control

- `F1`:  Toggles play/pause
- `F2`:  Pauses and steps back one frame
- `F3`:  Pauses and steps forward one frame
- `F4`:  Restarts playback at frame 0 (iTime = 0)
- `ESC`: Exit the application

If you are using the keyboard resouce, be sure to avoid these keys. Additionally, you may want to avoid making use of any of the function keys, as I may use these for other features in the future. Note that while toggling play/pause and restarting playback (F1 and F4) work as expected with audio/video resources, F2 and F3 (frame stepping) do not.

## Resources

### fragment shaders
- [Ray Marching and Signed Distance Functions](http://jamie-wong.com/2016/07/15/ray-marching-signed-distance-functions/)
- [Modeling with Signed Distance Functions](http://iquilezles.org/www/articles/distfunctions/distfunctions.htm)
- [Ray Marching Distance Fields](http://9bitscience.blogspot.com/2013/07/raymarching-distance-fields_14.html)
- [The Book of Shaders](https://thebookofshaders.com/)

### vertex shaders
- [vertexshaderart lessons](https://www.youtube.com/watch?v=mOEbXQWtP3M&list=PLC80qbPkXBmw3IR6JVvh7jyKogIo5Bi-d)
- [perspective projection matrix](http://www.songho.ca/opengl/gl_projectionmatrix.html)

## Inspiration

- [shadertoy](https://www.shadertoy.com)
- [vertexshaderart](https://www.vertexshaderart.com)
- [bonzomatic](https://github.com/Gargaj/Bonzomatic)
- [interactiveshaderformat](https://www.interactiveshaderformat.com/)
- [the book of shaders](https://thebookofshaders.com/)
- [https://shadertoyunofficial.wordpress.com/](https://shadertoyunofficial.wordpress.com/)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
