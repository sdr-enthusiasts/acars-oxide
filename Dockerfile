FROM ghcr.io/sdr-enthusiasts/docker-baseimage:rtlsdr

SHELL ["/bin/bash", "-o", "pipefail", "-c"]
COPY ./rootfs /
COPY ./bin/acars-oxide.armv7/acars-oxide /opt/acars-oxide.armv7
COPY ./bin/acars-oxide.arm64/acars-oxide /opt/acars-oxide.arm64
COPY ./bin/acars-oxide.amd64/acars-oxide /opt/acars-oxide.amd64

# hadolint ignore=DL3008,DL3003,SC1091
RUN set -x && \
    KEPT_PACKAGES=() && \
    TEMP_PACKAGES=() && \
    KEPT_PACKAGES+=(libzmq5) && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    "${KEPT_PACKAGES[@]}" \
    "${TEMP_PACKAGES[@]}"\
    && \
    # ensure binaries are executable
    chmod -v a+x \
    /opt/acars-oxide.armv7 \
    /opt/acars-oxide.arm64 \
    /opt/acars-oxide.amd64 \
    && \
    # remove foreign architecture binaries
    /rename_current_arch_binary.sh && \
    rm -fv \
    /opt/acars-oxide.* \
    && \
    # clean up
    apt-get remove -y "${TEMP_PACKAGES[@]}" && \
    apt-get autoremove -y && \
    rm -rf /src/* /tmp/* /var/lib/apt/lists/* && \
    # test
    /opt/acars-oxide --version
