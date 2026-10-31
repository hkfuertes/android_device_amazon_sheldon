# Cross toolchain for catplay_android: static armv7 musl binary (runs on Android as a root native daemon,
# no NDK/bionic needed). Mirrors catplay's own Dockerfile, minus x264/openssl/dbus/jpeg (TX-only daemon).
FROM debian:bookworm

RUN apt-get update && apt-get install -y \
  build-essential git wget ca-certificates autoconf automake libtool pkg-config \
  python3 cmake ninja-build perl libclang-dev clang \
  && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/richfelker/musl-cross-make.git /tmp/mcm && cd /tmp/mcm && \
  printf '%s\n' 'TARGET = arm-linux-musleabihf' 'OUTPUT = /opt/musl-cross' 'GCC_VER = 9.4.0' \
    'MUSL_VER = 1.2.5' 'LINUX_HEADERS_VER = 6.6.10' > config.mak && \
  make -j$(nproc) && make install && rm -rf /tmp/mcm

ENV PATH=/root/.cargo/bin:/opt/musl-cross/bin:$PATH
RUN wget -qO /tmp/rustup.sh https://sh.rustup.rs && sh /tmp/rustup.sh -y && \
  rustup target add armv7-unknown-linux-musleabihf

ENV SYSROOT=/opt/musl-cross/arm-linux-musleabihf
RUN git clone -b v1.0.27 https://github.com/libusb/libusb.git /tmp/libusb && cd /tmp/libusb && \
  sed -i "s/use_udev=yes/use_udev=no/" configure.ac && ./autogen.sh && \
  CC=arm-linux-musleabihf-gcc AR=arm-linux-musleabihf-ar RANLIB=arm-linux-musleabihf-ranlib \
  CFLAGS="-O2 -fPIC -march=armv7-a -mfpu=neon -mfloat-abi=hard" \
  ./configure --disable-udev --disable-examples-build --disable-shared --enable-static \
    --host=arm-linux-musleabihf --prefix=/usr && \
  make -j$(nproc) && make DESTDIR=$SYSROOT install && rm -rf /tmp/libusb

ENV PKG_CONFIG_ALLOW_CROSS=1 \
    PKG_CONFIG_SYSROOT_DIR=/opt/musl-cross/arm-linux-musleabihf \
    PKG_CONFIG_LIBDIR=/opt/musl-cross/arm-linux-musleabihf/usr/lib/pkgconfig \
    CC_armv7_unknown_linux_musleabihf=arm-linux-musleabihf-gcc \
    CXX_armv7_unknown_linux_musleabihf=arm-linux-musleabihf-g++ \
    AR_armv7_unknown_linux_musleabihf=arm-linux-musleabihf-ar \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-linux-musleabihf-gcc \
    CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_RUSTFLAGS="-C link-arg=-lgcc -C target-feature=+crt-static" \
    CFLAGS_armv7_unknown_linux_musleabihf="-O2 -fPIC -march=armv7-a -mfpu=neon -mfloat-abi=hard" \
    CARGO_HOME=/cargo
