FROM debian:bookworm-slim

ARG TARGETARCH
ARG HYPERION_VERSION=2.2.1
ENV DEBIAN_FRONTEND=noninteractive
ENV LD_LIBRARY_PATH=/usr/share/hyperion/lib

RUN set -eu; \
    case "${TARGETARCH}" in \
      amd64) package_sha256="8e30585049421297455387564f8df2c87603f0f8557bc95abca102be27e872b9" ;; \
      arm64) package_sha256="ee5e9eaeeb4a6bc69174b15dcb316de274cc77dab93fc5c370905e04ca2f056b" ;; \
      *) echo "Unsupported architecture: ${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    apt-get update; \
    apt-get install --yes --no-install-recommends \
      ca-certificates \
      curl \
      libcec6 \
      libexpat1 \
      libfontconfig1 \
      libglib2.0-0; \
    package="Hyperion-${HYPERION_VERSION}-Linux-${TARGETARCH}.deb"; \
    curl --fail --location --output "/tmp/${package}" \
      "https://github.com/hyperion-project/hyperion.ng/releases/download/${HYPERION_VERSION}/${package}"; \
    echo "${package_sha256}  /tmp/${package}" | sha256sum --check --strict; \
    apt-get install --yes --no-install-recommends "/tmp/${package}"; \
    rm -f "/tmp/${package}"; \
    rm -rf /var/lib/apt/lists/*; \
    useradd --create-home --home-dir /config --uid 10001 hyperion; \
    chown hyperion:hyperion /config

USER hyperion
VOLUME ["/config"]
EXPOSE 8090 19400 19444

ENTRYPOINT ["hyperiond", "--userdata", "/config"]
