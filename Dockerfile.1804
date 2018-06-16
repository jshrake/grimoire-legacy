FROM ubuntu:18.04

RUN apt-get update && apt-get install -yf curl \
    libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev gstreamer1.0-plugins-base \
    libegl1-mesa-dev libgles2-mesa-dev libsdl2-dev
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
WORKDIR /home/grimoire
COPY . /home/grimoire
RUN /root/.cargo/bin/cargo --version
RUN /root/.cargo/bin/cargo build
RUN /root/.cargo/bin/cargo test --all --verbose
RUN /root/.cargo/bin/cargo doc --verbose