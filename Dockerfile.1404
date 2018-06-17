FROM ubuntu:14.04

RUN apt-get update && apt-get install -yf build-essential wget xz-utils git curl \
    bison flex glib2.0-dev \
    libegl1-mesa-dev libgles2-mesa-dev libsdl2-dev
RUN wget https://gstreamer.freedesktop.org/src/gstreamer/gstreamer-1.14.1.tar.xz && \
    tar xf gstreamer-1.14.1.tar.xz && \
    cd gstreamer-1.14.1 && ./configure --prefix=/usr && make && make install && \
    cd .. && rm -rf gstreamer-1.14.1*
RUN wget https://gstreamer.freedesktop.org/src/gst-plugins-base/gst-plugins-base-1.14.1.tar.xz && \
    tar xf gst-plugins-base-1.14.1.tar.xz && \
    cd gst-plugins-base-1.14.1 && ./configure --prefix=/usr && make && make install && \
    cd .. && rm -rf gst-plugins-base-1.14.1*
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
WORKDIR /home/grimoire
COPY . /home/grimoire
RUN /root/.cargo/bin/cargo build