# FAQ

## How can I control playback?

- **ESC**: close the window and exit the application
- **F1**: toggle play/pause (freezes/unfreezes iTime and iFrame)
- **F2**: decrement iFrame by 1, and iTime by 16ms
- **F3**: increment iFrame by 1, and iTime by 16ms
- **F4**: reset iTime and iFrame to 0

The controls are mapped to the function keys because I believe that these keys are the least likely to also
be used in a shader using the keyboard resource. It is likely that I will make use of the remaning function keys
for future features. Users should avoid using the function keys in shaders that require keyboard input.

## I'm having an issue with webcam, audio, video, microphone, kinect2, or openni2 input!

grimoire depends on [GStreamer](https://GStreamer.freedesktop.org/) to provide webcam, audio, video, microphone, kinect2, and openni2 input. I am a novice GStreamer user and there may be some issues in the code paths that support these features. If you encounter any issues, please run your program with environment variable `GST_DEBUG=3` set, and [file an issue](https://github.com/jshrake/grimoire/issues/new) with the logs attached and I will do my best to resolve the issue.

The kinect2 and openni2 examples are experimental and rely on custom forks of GStreamer plugins that you will need to clone, build, and install from source. See below for insturctions on how to do this. I make no promises that these will work on your machine. As of this comment, they are only known to work on my MacOS machine. Still, if you encounter any issue, [file it](https://github.com/jshrake/grimoire/issues/new)!

### How do I run the [kinect2 example](https://github.com/jshrake/grimoire-examples/blob/master/kinect2.glsl)?

Here are the steps I took to use the `freenect2src` GStreamer element provided by [https://github.com/lubosz/gst-plugins-vr](https://github.com/lubosz/gst-plugins-vr) on MacOS. Change set: (https://github.com/jshrake/gst-plugins-vr/compare/master...jshrake:grimoire?expand=1)

Build and install libfreenect2 from source:
```console
$ brew install libusb
$ git clone https://github.com/OpenKinect/libfreenect2.git
$ cd libfreenect2
$ cmake -H. -Bbuild -DCMAKE_BUILD_TYPE=Release
$ cmake --build build
$ sudo make -C build install
```

Build and install [my fork of gst-plugins-vr](https://github.com/jshrake/gst-plugins-vr/tree/grimoire).
```console
$ git clone https://github.com/jshrake/gst-plugins-vr
$ cd gst-plugins-vr
$ git checkout grimoire
$ ./configure
$ make
$ sudo make install
```

Run the example:
```console
$ LIBFREENECT2_PIPELINE=cl cargo run -- examples/kinect2.glsl
```

If the `cl` libfreenect2 pipeline doesn't work, try `cpu` or `cuda`. grimoire is currently not compatible with the default `gl` pipeline.

## How do I run the [openni2.glsl example](https://github.com/jshrake/grimoire-examples/blob/master/openni2.glsl)?

**This shader uses an experimental GStreamer plugin than can cause grimoire to segfault**

Here are the steps I took to use the `openni2src` GStreamer element provided by [gst-plugins-bad](https://github.com/GStreamer/gst-plugins-bad/tree/master/ext/openni2) on MacOS. I [forked gst-plugins-bad and made changes to the openni2src element](https://github.com/jshrake/gst-plugins-bad/compare/1.14.1...jshrake:grimoire-1.14.1). Note that using this plugin causes grimoire to sefault on TOML configuration changes.

Build and install libfreenect2 from source:
```console
$ brew install libusb
$ git clone https://github.com/OpenKinect/libfreenect2.git
$ cd libfreenect2
$ cmake -H. -Bbuild -DCMAKE_BUILD_TYPE=Release
$ cmake --build build
$ sudo make -C build install
```

Install openni2 (https://github.com/totakke/homebrew-openni2) and manually create a pkg-config file for libopenni2:
```console
$ brew install openni2
$ vim /usr/local/lib/pkgconfig/libopenni2.pc
prefix=/usr/local
exec_prefix=${prefix}
libdir=${exec_prefix}/lib/ni2
includedir=${prefix}/include/ni2

Name: OpenNI2
Description: A general purpose driver for all OpenNI cameras.
Version: 2.2.0.0
Cflags: -I${includedir}
Libs: -L${libdir} -lOpenNI2 -L${libdir}/OpenNI2/Drivers -lOniFile -lPS1080
```

Build and install [my fork of gst-plugins-bad](https://github.com/jshrake/gst-plugins-bad/tree/grimoire-1.14.1) from source. Ensure that the openni2 plugin builds! If it doesn't, ensure `pkg-config --debug libopenni2` returns something sane.
```console
$ git clone https://github.com/jshrake/gst-plugins-bad
$ cd gst-plugins-bad
$ git checkout grimoire-1.14.1
$ ./configure
$ make
$ sudo make install
```

Run the example:
```console
$ LIBFREENECT2_PIPELINE=cl cargo run -- examples/openni2.glsl
```

If the `cl` libfreenect2 pipeline doesn't work, try `cpu` or `cuda`. grimoire is currently not compatible with the default `gl` pipeline.

## Where can I find images for cube maps?

http://www.custommapmakers.org/skyboxes.php contains many high resolution skyboxes

## Where can I find volumetric data?

You can download datasets from http://schorsch.efi.fh-nuernberg.de/data/volume/. At this time, grimoire does not support loading in the pvm file format. Instead, users will need to download https://sourceforge.net/projects/volren/, build the source, and run a tool that converts the pvm data into a raw format. I was able to successfully build the project and run the tool as follows on MacOS:

```
$ wget https://downloads.sourceforge.net/project/volren/VIEWER-5.2.zip
$ unzip VIEWER-5.2.zip
$ cd viewer
$ cmake -H. -Bbuild -DCMAKE_BUILD_TYPE=Release
$ cmake --build build
$ wget http://schorsch.efi.fh-nuernberg.de/data/volume/Foot.pvm
$ ./build/tools/pvm2raw Foot.pvm Foot.raw
reading PVM file
found volume with width=256 height=256 depth=256 components=1
and data checksum=4FAD56F0
```

Take note of width, height, depth, and components values, as you'll need to specify these in the resource configuration.