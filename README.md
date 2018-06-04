# grimoire
**grimoire is a command-line tool for creating shader demos on Windows, MacOS, and Linux**

<img src="https://thumbs.gfycat.com/CriminalEnergeticBird-size_restricted.gif" width="300" height="200" /><img src="https://thumbs.gfycat.com/LikableJoyfulAsianelephant-size_restricted.gif" width="300" height="200" />

Create interactive art, make games, learn computer graphics, have fun!

- [What?](#what)
- [How?](#how)
- [Install](#install)
    - [Windows](#windows)
    - [MacOS](#macos)
    - [Linux](#linux)
- [Inspiration](#inspiration)
- [Dual licensed under MIT & Apache 2.0](#license)

## What?

grimoire is best described as a native [shadertoy](https://www.shadertoy.com/) clone with the following features:

- Live code your shader demo in a single file in your editor of choice
- Shader inputs: [images](https://github.com/jshrake/grimoire-examples/blob/master/image.glsl), [videos](https://github.com/jshrake/grimoire-examples/blob/master/video.glsl), [audio](https://github.com/jshrake/grimoire-examples/blob/master/audio.glsl), [cubemaps](https://github.com/jshrake/grimoire-examples/blob/master/cubemap.glsl), [volumetric data](https://github.com/jshrake/grimoire-examples/blob/master/volume.glsl), [webcam](https://github.com/jshrake/grimoire-examples/blob/master/webcam.glsl), [mouse](https://github.com/jshrake/grimoire-examples/blob/master/shadertoy-mouse.glsl), [time of day](https://github.com/jshrake/grimoire-examples/blob/master/shadertoy-time.glsl), [keyboard](https://github.com/jshrake/grimoire-examples/blob/master/shadertoy-keyboard-debug.glsl), [microphone](https://github.com/jshrake/grimoire-examples/blob/master/microphone.glsl), [kinect2](https://github.com/jshrake/grimoire-examples/blob/master/kinect2.glsl), and [openni2 devices](https://github.com/jshrake/grimoire-examples/blob/master/openni2.glsl)
- [Multi-pass rendering](https://github.com/jshrake/grimoire-examples/blob/master/multi-pass-feedback.glsl)
- [Multiple render targets](https://github.com/jshrake/grimoire-examples/blob/master/multiple-render-targets.glsl)
- [Vertex shaders with custom geometry](https://github.com/jshrake/grimoire-examples/blob/master/vsa-twisted-torus.glsl)
- [Shadertoy compatibility](https://github.com/jshrake/grimoire-examples/blob/master/shadertoy-new.glsl)
- Cross-platform (Windows, MacOS, Linux)
    * [SDL2](https://www.libsdl.org/index.php) for window and input handling
    * [GStreamer](https://GStreamer.freedesktop.org/) for video, webcam, audio, microphone, and kinect2 inputs
    * OpenGL 3.3+, but uses a subset of OpenGL accessible from GLES 3.0

[Install now](#install) and [learn by example](https://github.com/jshrake/grimoire-examples)!

## How?

- The only required input to grimoire is a single [GLSL](https://en.wikipedia.org/wiki/OpenGL_Shading_Language) file with [TOML](https://github.com/toml-lang/toml) configuration embedded in a comment block. You author this file in your editor of choice.
- Your configuration defines a list of ordered rendering passes. Your GLSL code defines the vertex and fragment shader main function for each pass. grimoire inserts several `#define` statements before compiling your GLSL code depending on the shader type under compilation and the pass. Your GLSL code conditions on these statements in `#ifdef` blocks to ensure only one main function is compiled per shader.
- Your configuration defines named texture inputs that the pass configuration references to associate uniform sampler names with inputs. grimoire inserts the correct uniform sampler declarations into your code based on the input type.
- As you edit and save the file, grimoire reloads resources and recompiles the shader programs live. As you edit and save file backed resources referenced by the configuration, grimoire reloads the texture data. You never need to restart grimoire once it's running, even on misconfigurations or GLSL compilation errors.
- Compatibility with shadertoy is a feature. Users should be able to to copy-paste code between grimoire and shadertoy with minimal tinkering on either side.

Want to learn more? [Read the spec](spec.md)!

## Install

You need to build and install grimoire from source using [rust](https://www.rust-lang.org/en-US/install.html) and install the required system dependencies:

- [SDL2](https://wiki.libsdl.org/Installation)
- [GStreamer](https://GStreamer.freedesktop.org/documentation/installing/index.html)

### Windows

This is a really rough experience right now and the following steps may not work for you without further tinkering:

- Download and run [msys2](https://www.msys2.org/)
- Install the required dependencies
- Manually copy the SDL2 and GStreamer DLLs to the top-level grimoire source directory (the one containing Cargo.toml) before running

```console
$ pacman -S mingw-w64-x86_64-pkg-config mingw-w64-x86_64-SDL2 mingw-w64-x86_64-GStreamer \
mingw-w64-x86_64-gst-plugins-base mingw-w64-x86_64-gst-plugins-good \
mingw-w64-x86_64-gst-plugins-bad mingw-w64-x86_64-gst-plugins-ugly \
mingw-w64-x86_64-gst-libav
$ rustup target add x86_64-pc-windows-gnu
$ git clone https://github.com/jshrake/grimoire
$ cd grimoire
$ cargo build --release
```

Breadcrumbs:
- https://github.com/sdroege/GStreamer-rs#windows

### MacOS

This is my primary development enviornment and should be your path of least resistance:

```console
$ brew install sdl2 gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-plugins-ugly gst-libav
$ git clone https://github.com/jshrake/grimoire
$ cd grimoire
$ cargo install --path .
```

### Linux

I haven't tested this yet, but it should work with minimal effort:

```console
$ apt-get install libsdl2-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly \
gstreamer1.0-libav
$ git clone https://github.com/jshrake/grimoire
$ cd grimoire
$ cargo install --path .
```

## Inspiration



- [shadertoy](https://www.shadertoy.com)
- [vertexshaderart](https://www.vertexshaderart.com)
- [interactiveshaderformat](https://www.interactiveshaderformat.com/)
- [handmade hero](https://handmadehero.org/)
- [https://shadertoyunofficial.wordpress.com/](https://shadertoyunofficial.wordpress.com/)

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
