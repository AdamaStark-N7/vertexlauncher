ARG BASE_IMAGE=docker.io/library/debian:bookworm
FROM ${BASE_IMAGE}

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      ca-certificates \
      binutils \
      cpio \
      dnf \
      dnf-plugins-core \
      flatpak \
      flatpak-builder \
      imagemagick \
      rpm2cpio \
    && rm -rf /var/lib/apt/lists/*
