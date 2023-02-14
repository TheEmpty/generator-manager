FROM rust:alpine

# Packages
ENV BUILD_PACKAGES "pkgconfig"
ENV DEP_PACKAGES "gcc openssl-dev musl-dev"
RUN apk add --no-cache ${BUILD_PACKAGES} ${DEP_PACKAGES}

# Code
RUN mkdir -p /code /generator-manager
COPY Cargo.toml /code/.
COPY Cargo.lock /code/.
COPY src /code/src

# Build vars
ENV BINARY "generator-manager"
# Believe this requirement stems from reqwest
ENV RUSTFLAGS "-Ctarget-feature=-crt-static"

# Compile && Cleanup
RUN cd /code \
  && export RUSTC_VERSION="$(rustc -V)" \
  && export DATE_TIME="$(date -u)" \
  && cargo build --release --verbose \
  && cp target/release/${BINARY} /opt/app \
  && rm -fr /code \
  && apk --purge del ${BUILD_PACKAGES}

COPY resources /generator-manager

# Runtime env
ENV RUST_LOG=generator_manager=debug
ENV RUST_BACKTRACE=1
ENV ROCKET_TEMPLATE_DIR=/generator-manager/templates/
ENV ROCKET_STATIC_DIR=/generator-manager/static/
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT 80

ENTRYPOINT ["/opt/app"]
