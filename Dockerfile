# Runtime-only image that expects a prebuilt binary to be present in the build context.
# The GitHub Actions workflow builds the binary for each architecture and places it under
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    binutils \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# TARGETARCH is provided by buildx (e.g. amd64, arm64)
ARG TARGETARCH
ARG APP_NAME
COPY ./target/release/qrud /app/qrud

EXPOSE 3000

CMD ["/bin/sh", "-c", "exec /app/qrud"]
