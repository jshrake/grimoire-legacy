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
    - [Vertex shader](#fragment)
    - [Fragment shader](#vertex)


The only required input to grimoire is a single [GLSL](https://en.wikipedia.org/wiki/OpenGL_Shading_Language) file with [TOML](https://github.com/toml-lang/toml) configuration embedded in a comment block. This document describes the configuration format and the GLSL code grimoire inserts into your shader before compilation. 

# Configuration

- The configuration must be defined in the first comment block of the file
- The configuration is composed of two main parts: named resources and an ordered list of passes
- Resources consist of named texture inputs, and named buffers (that are also used as texture inputs)
- Passes draw into a buffer by specifying the `buffer` key. If no `buffer` key is present, the pass draws to the default framebuffer
- Passes configure uniform samplers for use in the shader code by specifying the desired uniform name as a key, and a resource name for the value
    - You can give your uniforms any name, except "buffer", "draw", "blend", "depth", "clear"
    - Uniform declarations are automatically inserted into your code before compilation
- Passes can optionally configure the primitive type (triangles, points, lines) and count to draw, blending, depth testing, and the clear color
    - Each of these defaults to sensible values for a shadertoy-like demo where each pass draws a full-screen quad
- If no configuration is defined, a single pass is defined automatically to draw a full-screen quad to the default framebuffer.

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

Your GLSL code defines the vertex and fragment shader main function for all the passes. grimorie inserts additional code above and below your GLSL code before compiling the vertex and fragment shader for each pass. [Default main definitions](./src/default_shader_footer.glsl) are inserted for both the vertex and fragment shader. The default vertex shader positions a full-screen quad, and the default fragment shader is the shadertoy image shader and expects the user to define `void mainImage(out vec4 fragColor, in vec2 fragCoord)`. You can disable the default main definition for your pass by specifying `#define GRIM_OVERRIDE_MAIN` in your code.

## Fragment

The fragment shader for each pass is generated by concatenating the following list of strings:

- A `#version` statement appropriate for the `gl` argument passed at the command-line
- `#define GRIM_FRAGMENT`
- `#define GRIM_FRAGMENT_PASS_%d`, where `%d` is the zero-based index of the pass under compilation
- `#define GRIM_PASS_%d`, equivalent to `GRIM_FRAGMENT_PASS_%d`
- Default uniform declarations available to all passes, see [header.glsl](./src/header.glsl)
- Uniform sampler declarations generated from the pass configuration, see [uniform insertion](#uniform-insertion)
- `#line 1 0`
- Your shader code
- Default main definitions, see [footer.glsl](./src/footer.glsl). Disable by specifying `#define GRIM_OVERRIDE MAIN`

### Useful patterns

- A single pass shader requires no GLSL preprocessor directives

```glsl
...
void mainImage (out vec4 fragColor, in vec2 fragCoord) {
    ...
}
```

- Mutli-pass shaders need to isolate `mainImage` definitions in `#ifdef GRIM_PASS_%d` blocks

```glsl
...
#ifdef GRIM_PASS_0
void mainImage (out vec4 fragColor, in vec2 fragCoord) {
}
#endif
#ifdef GRIM_PASS_1
void mainImage (out vec4 fragColor, in vec2 fragCoord) {
}
#endif
```

- [Writing to multiple render targets in a single pass, and referencing each target in another pass](https://github.com/jshrake/grimoire-examples/blob/master/multiple-render-targets.glsl)

## Vertex

The vertex shader for each pass is generated by concatenating the following list of strings:

- A `#version` statement appropriate for the `gl` argument passed at the command-line
- `#define GRIM_VERTEX`
- `#define GRIM_VERTEX_PASS_%d`, where `%d` is the zero-based index of the pass under compilation
- Default uniform declarations available to all passes, see [header.glsl](./src/header.glsl)
- Uniform sampler declarations generated from the pass configuration, see [uniform insertion](#uniform-insertion)
- `#line 1 0`
- Your shader code
- Default main definitions, see [footer.glsl](./src/footer.glsl). Disable by specifying `#define GRIM_OVERRIDE MAIN`

### Useful patterns

- Override the default vertex shader for all passes:

```glsl
...
#ifdef GRIM_VERTEX
#define GRIM_OVERRIDE_MAIN
void main() {...}
#endif
...
```

- Override the default vertex shader of a specific pass:

```glsl
...
#ifdef GRIM_VERTEX_PASS_%d
#define GRIM_OVERRIDE_MAIN
void main() {...}
#endif
...
```

- Override the default vertex shader for all passes and further override a specific pass:

```glsl
...
#ifdef GRIM_VERTEX_PASS_%d
#define GRIM_OVERRIDE_MAIN
#define OVERRIDE_OUR_DEFAULT
void main() {...}
#endif
...
#if defined(GRIM_VERTEX) && !defined(OVERRIDE_OUR_DEFAULT)
#define GRIM_OVERRIDE_MAIN
void main() {...}
#endif
...
```
