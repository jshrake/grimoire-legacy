# File Format and Runtime Behavior

- [Configuration](#configuration)
    - [Resources](#resources)
        * [Buffer](#buffer)
        * [Image](#image)
        * [Texture2D](#texture2d)
        * [Texture3D](#texture3d)
        * [Cubemap](#cubemap)
        * [Keyboard](#keyboard)
        * [Webcam](#webcam)
        * [Microphone](#microphone)
        * [Video](#video)
        * [Audio](#audio)
        * [GStreamer Pipeline](#pipeline)
    - [Passes](#passes)
- [GLSL](#glsl)


The only required input to grimoire is a [TOML](https://github.com/toml-lang/toml) configuration file. This configuration file defines a series of draw passes and resources. This document describes the schema for how you define these draw passes and resources, and documents the GLSL code that's automatically prepended to your shader code at runtime.

# Configuration

- The configuration is composed of two main parts: named resources and an ordered list of passes
- A resource is an umbrella term for anything that can be converted into a texture and sampled in a shader. Examples include images, video, webcam, keyboard input, g-streamer pipelines, and framebuffer objects.
- A pass defines a single draw call, including vertex and fragment shaders, the number of primitives (points, lines, triangles, triangle fan, etc.), draw target (A named resource `buffer`), and uniform names to bind to declared resources.
- Passes draw into a buffer by specifying the `buffer` key. If no `buffer` key is present, the pass draws to the default framebuffer
- Passes configure uniform samplers for use in the shader code by specifying the desired uniform name as a key, and a resource name for the value
    - You can give your uniforms any name, except "buffer", "draw", "blend", "depth", "clear"
    - Uniform declarations are automatically inserted into your code before compilation
- Passes configure the primitive type (triangles, points, lines) and count to draw, blending, depth testing, and the clear color

## Resources

A resource is a [TOML table](https://github.com/toml-lang/toml#user-content-table) that configures a texture object or a framebuffer with color attachments. Below is a list of all resource scehmas. Note that all relative paths are relative to the shader input file.

### Buffer
Configures a framebuffer object that a pass can draw to. This is the only resource type that can be referenced by the pass `buffer` configuration.

- **buffer=bool**, Required, the value is ignored.
- **width=u32**, Optional, defauls to the window width
- **height=u32** , Optional, defaults to the window height
- **attachments=u32**, Optional, defaults to 1
- **format=string{"u8", "f16", "f32"}**: Optional, defaults to "f32"

### Image
- **image=string**: Required, relative path to an image file. Supports [png, jpeg, gif, bmp, ico, tiff, webp, pnm](https://github.com/PistonDevelopers/image#21-supported-image-formats)
- **flipv=bool**: Optional, flip the image vertically before uploading to the GPU, defaults to true
- **fliph=bool**: Optional, flip the image horizontally before uploading to the GPU, defaults to false

### Texture2D
- **texture2D=string**: Required, relative path to a file containing 2D texture data
- **width=u32**: Required, the texture width
- **height=u32**: Required, the texture height
- **format=string**: Required, the texture format, accepts the following strings: "ru8", "rf16", "rf32", "rgu8", "rgf16", "rgf32", "rgbu8", "rgbf16", "rgbf32", "rgbau8", "rgbaf16", "rgbaf32", "bgru8", "bgrf16", "bgrf32", "bgrau8", "bgraf16", "bgraf32"

### Texture3D
- **texture3D=string**: Required, relative path to a file containing 3D texture data
- **width=u32**: Required, the texture width
- **height=u32**: Required, the texture height
- **depth=u32**: Required, the texture depth
- **format=string**: Required, the texture format, accepts the following strings: "ru8", "rf16", "rf32", "rgu8", "rgf16", "rgf32", "rgbu8", "rgbf16", "rgbf32", "rgbau8", "rgbaf16", "rgbaf32", "bgru8", "bgrf16", "bgrf32", "bgrau8", "bgraf16", "bgraf32"

### Cubemap
- **left=string**: Required, relative path to an image file for the left cubemap face
- **right=string**: Required, relative path to an image file for the right cubemap face
- **front=string**: Required, relative path to an image file for the front cubemap face
- **back=string**: Required, relative path to an image file for the back cubemap face
- **top=string**: Required, relative path to an image file for the top cubemap face
- **bottom=string**: Required, relative path to an image file for the bottom cubemap face
- **flipv=bool**: Optional, flip the image vertically before uploading to the GPU, defaults to true
- **fliph=bool**: Optional, flip the image horizontally before uploading to the GPU, defaults to false

Each face supports the same file formats as [image](#image) input.

### Keyboard
- **keyboard=bool**: Required, the value is ignored.

### Webcam
- **webcam=bool**: Required, the value is ignored.

### Microphone
- **microphone=bool**: Required, the value is ignored.

### Video
- **video=string**: Required, relative path to a video file OR a uri. File support depends on your GStreamer installation. Uses [playbin](https://gstreamer.freedesktop.org/data/doc/gstreamer/head/gst-plugins-base-plugins/html/gst-plugins-base-plugins-playbin.html) internally. Users can use `playbin2` and `playbin3` by defining the enviornment variables `USE_PLAYBIN2=1 ` and `USE_PLAYBIN3=1 `, respectively.

### Audio
- **audio=string**: Required, relative path to an audio file OR a uri. File support depends on your GStreamer installation. Uses [uridecodebin](https://gstreamer.freedesktop.org/data/doc/gstreamer/head/gst-plugins-base-plugins/html/gst-plugins-base-plugins-uridecodebin.html) internally.

### Pipeline
- **pipeline=string**: Required, a GStreamer [gst-launch pipeline description](https://gstreamer.freedesktop.org/documentation/tools/gst-launch.html). grimoire assumes that the pipeline description contains an appsink element with name appsink and that the pipeline produces samples with video caps.

## Passes

Passes are defined as an [array of tables](https://github.com/toml-lang/toml#array-of-tables) and are drawn in the order listed in the configuration. 

- **buffer=string**: Optional, the buffer to draw into. If not specified, the pass draws to the default framebuffer
- **draw={mode=string{"triangles", "points", ...}, count=u32}**: configures the draw primitive and number of vertices to draw, defaults to mode="triangles", count=1. Valid mode values: "triangles", "points", "lines", "triangle-fan", "triangle-strip", "line-strip", "line-loop"
- **depth=string{"less",...}**: depth testing, defaults to disabled. Valid values: "never", "less", "equal", "less-equal", "greater", "not-equal", "greater-equal", "always"
- **blend={src=string{"one",..}, dest=string{"one-minus-src-alpha",..}}**: blend functions, defaults to disabled. Valid src and dest values: "zero", "one", "src-color", "one-minus-src-color", "dst-color", "one-minus-dst-color", "src-alpha", "one-minus-src-alpha", "dst-alpha", "one-minus-dst-alpha"
- **clear=[f32;4]**: configures the clear color for the pass, defaults to [0.0, 0.0, 0.0, 1.0]

All other key-value pairs associate a uniform sampler with a resource. grimoire uses the key name to generate uniform sampler declarations that are inserted into your code. The valid values are:

- **samplerName=string**: a resource name, defaults to wrap="repeat", filter="mipmap" (filter="linear" for Texture3D inputs)
- **samplerName={resource=string, wrap="clamp","repeat", filter="linear","nearest","mipmap"}**: requires resource, defaults to wrap="repeat", filter="mipmap"

### Uniform Insertion

For each uniform sampler resource pairs in pass, grimoire inserts the following uniform declarations into your shader code before compilation:

- `uniform SAMPLERTYPE_FROM_VAL NAME`: The texture sampler
- `uniform vec3 NAME_Resolution`: The resolution of the texure resource, z contains the aspect ratio
- `uniform float NAME_Time`: The playback time  of the texture resource

Use names like `iChannel0`, `iChannel1`, ... `iChannelN` to make it easier to copy-paste your shader code into shadertoy.

# GLSL

Grimoire prepends the following GLSL code to your shader code:

- The `#version` directive. You can explicitly control this value by the `--gl` command-line argument.
- [Uniform declarations required by grimoire](./grimoire/shaders/shadertoy_uniforms.glsl) for data such as current time, current frame, mouse state, window resolution, etc.. At the time of writing, the uniform names match the uniform names used on shadertoy.
- Uniform sampler declarations of the appropriate type for the uniforms defined in the pass configuration.