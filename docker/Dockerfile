FROM debian:bullseye AS cc

RUN useradd -m ccuser
RUN mkdir -p /opt/cross
RUN chown -R ccuser /opt/cross

ENV PATH="/opt/cross/bin:${PATH}"

# install dependencies of GCC and other basic utils
RUN apt-get update && apt-get install -y --no-install-recommends \
	libmpfr-dev libmpc-dev libgmp-dev \
	autotools-dev autoconf file rsync flex bison binutils gawk gcc g++ make python3 \
	ca-certificates curl xz-utils lbzip2 bzip2 \
	&& rm -rf /var/lib/apt/lists/*


USER ccuser
WORKDIR "/tmp"

ARG GCC_VERSION

# download and extract GCC
RUN echo "downloading gcc version ${GCC_VERSION}" && curl -f -O https://gcc.gnu.org/pub/gcc/releases/gcc-${GCC_VERSION}/gcc-${GCC_VERSION}.tar.xz
RUN tar -xf gcc-${GCC_VERSION}.tar.xz

RUN echo "downloading isl 0.18" && curl -f -O https://gcc.gnu.org/pub/gcc/infrastructure/isl-0.18.tar.bz2
RUN tar -xf isl-0.18.tar.bz2

RUN echo "downloading cloog1 0.18.1" && curl -f -O https://gcc.gnu.org/pub/gcc/infrastructure/cloog-0.18.1.tar.gz
RUN tar -xf cloog-0.18.1.tar.gz

RUN ln -s ../isl-0.18 gcc-8.5.0/isl
RUN ln -s ../cloog-0.18.1 gcc-8.5.0/cloog

ARG BINUTILS_VERSION

# download and extract binutils
RUN echo "downloading binutils version ${BINUTILS_VERSION}" && curl -f -O https://ftp.gnu.org/gnu/binutils/binutils-${BINUTILS_VERSION}.tar.xz
RUN tar xf binutils-${BINUTILS_VERSION}.tar.xz

ARG TARGET_GCC

# build and install binutils
RUN mkdir -p /tmp/build-binutils
WORKDIR "/tmp/build-binutils"
RUN echo "configuring binutils version ${BINUTILS_VERSION} with target ${TARGET_GCC}" && ../binutils-${BINUTILS_VERSION}/configure --prefix=/opt/cross --target=${TARGET_GCC} --enable-gold --disable-nls
RUN make -j8 && make install

ARG LINUX_SERIES
ARG LINUX_VERSION
ARG TARGET_LINUX

# download and extract linux headers
WORKDIR "/tmp"
RUN echo "downloading linux version ${LINUX_VERSION}" && curl -f -O https://mirrors.edge.kernel.org/pub/linux/kernel/v${LINUX_SERIES}/linux-${LINUX_VERSION}.tar.xz
RUN tar xf linux-${LINUX_VERSION}.tar.xz

# install linux headers
WORKDIR "/tmp/linux-${LINUX_VERSION}"
RUN make ARCH=${TARGET_LINUX} INSTALL_HDR_PATH=/opt/cross/${TARGET_GCC} headers_install

ARG GCC_CONFIGURE_FLAGS
ENV GCC_CONFIGURE_FLAGS=${GCC_CONFIGURE_FLAGS}

# build and install gcc
RUN mkdir -p /tmp/build-gcc
WORKDIR "/tmp/build-gcc"
RUN ../gcc-${GCC_VERSION}/configure --prefix=/opt/cross --target=${TARGET_GCC} --enable-languages=c,c++ --disable-bootstrap --disable-nls ${GCC_CONFIGURE_FLAGS}
RUN make -j8 all-gcc
RUN make -j8 install-gcc

ARG GLIBC_VERSION

# download and extract glibc
WORKDIR "/tmp"
RUN curl -O https://ftp.gnu.org/gnu/glibc/glibc-${GLIBC_VERSION}.tar.xz
RUN tar xf glibc-${GLIBC_VERSION}.tar.xz

# build glibc stage 1
RUN mkdir -p /tmp/build-glibc
WORKDIR "/tmp/build-glibc"

ARG CPPFLAGS
ENV CPPFLAGS=${CPPFLAGS}

RUN ../glibc-${GLIBC_VERSION}/configure --prefix=/opt/cross/${TARGET_GCC} --build=$MACHTYPE --host=${TARGET_GCC} --target=${TARGET_GCC} --with-headers=/opt/cross/${TARGET_GCC}/include libc_cv_forced_unwind=yes || cat config.log
RUN make install-bootstrap-headers=yes install-headers
RUN make -j8 csu/subdir_lib
RUN install csu/crt1.o csu/crti.o csu/crtn.o /opt/cross/${TARGET_GCC}/lib
RUN ${TARGET_GCC}-gcc ${CPPFLAGS} -nostdlib -nostartfiles -shared -x c /dev/null -o /opt/cross/${TARGET_GCC}/lib/libc.so
RUN touch /opt/cross/${TARGET_GCC}/include/gnu/stubs.h

# build libgcc
WORKDIR "/tmp/build-gcc"
RUN make -j8 all-target-libgcc
RUN make -j8 install-target-libgcc

# build glibc stage 2
WORKDIR "/tmp/build-glibc"
RUN make -j8 && make install

USER root

ARG TARGET_DEBIAN
ENV TARGET_DEBIAN=${TARGET_DEBIAN}

# install dependencies of GCC and plane system
RUN dpkg --add-architecture ${TARGET_DEBIAN}
RUN apt-get update && apt-get install -y \
	libglib2.0-dev:${TARGET_DEBIAN} libssl-dev:${TARGET_DEBIAN} \
	libgtk-3-dev:${TARGET_DEBIAN} libasound2-dev:${TARGET_DEBIAN} libcairo2-dev:${TARGET_DEBIAN} \
	&& rm -rf /var/lib/apt/lists/*

# # install pango 1.44
# WORKDIR "/tmp/libpango"
# RUN curl -f -O http://ftp.gnome.org/pub/gnome/sources/pango/1.44/pango-1.44.7.tar.xz
# RUN tar xf pango-1.44.7.tar.xz
# WORKDIR "/tmp/libpango/pango-1.44.7"
# RUN mkdir build && cd build && meson --prefix=/usr --sysconfdir=/etc .. && ninja && ninja install

USER ccuser
WORKDIR "/home/ccuser"

# install rust toolchain
ARG TARGET_RUST
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --default-toolchain 1.65
ENV PATH="/home/ccuser/.cargo/bin:${PATH}"
RUN rustup target add ${TARGET_RUST}

# add rust config
ARG TARGET_PKGCONFIG
RUN mkdir -p .cargo && printf "[target.${TARGET_RUST}]\nrustflags=[\"-C\",\"link_args=-Wl,-rpath-link,/usr/lib/${TARGET_PKGCONFIG}/:/lib/${TARGET_PKGCONFIG}\",\"--cfg\",\"tokio_unstable\"]\nlinker = \"/opt/cross/bin/${TARGET_GCC}-gcc\"\n" > .cargo/config.toml
RUN cat .cargo/config.toml

# run
VOLUME /app
WORKDIR "/app"

# add env vars
ENV CFLAGS=${CPPFLAGS} CC=${TARGET_GCC}-gcc PKG_CONFIG_SYSROOT_DIR= PKG_CONFIG_PATH=/usr/lib/${TARGET_PKGCONFIG}/pkgconfig TARGET_RUST=${TARGET_RUST}
CMD cargo build --target ${TARGET_RUST}
