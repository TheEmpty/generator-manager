FROM rust:1.63-slim-buster

RUN apt-get update
RUN apt-get install -y gcc libssl-dev pkg-config
RUN mkdir -p /tmp/generator-manager
COPY src /tmp/generator-manager/src
COPY Cargo.toml /tmp/generator-manager

RUN cd /tmp/generator-manager \
  && cargo build --release --verbose \
  && cp target/release/generator-manager /opt \
  && rm -fr /src

ENV RUST_LOG=generator_manager=debug
ENV RUST_BACKTRACE=1

ENTRYPOINT ["/opt/generator-manager"]
