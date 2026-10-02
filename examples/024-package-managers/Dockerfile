# Container image from PREBUILT static binaries (no compilation in Docker – fast, same artifact
# as the archives). Build context: the dist directory created by package.sh.
#   docker buildx build --platform linux/amd64,linux/arm64 \
#     -f examples/024-package-managers/Dockerfile -t ghcr.io/example-org/platctl-pkg:1.2.0 dist/platctl-pkg-v1.2.0
ARG VERSION=1.2.0

# Stage 1: pick the archive matching the image platform. TARGETARCH (amd64/arm64) is set by buildx.
FROM alpine:3 AS pick
ARG VERSION
ARG TARGETARCH
COPY *.tar.gz /archives/
RUN case "$TARGETARCH" in \
      amd64) arch=x86_64 ;; \
      arm64) arch=aarch64 ;; \
      *) echo "unsupported arch $TARGETARCH" && exit 1 ;; \
    esac \
 && tar -xzf /archives/platctl-pkg-v${VERSION}-${arch}-unknown-linux-musl.tar.gz -C /tmp \
 && mv /tmp/platctl-pkg-v${VERSION}-${arch}-unknown-linux-musl/platctl-pkg /platctl-pkg

# Stage 2: empty image + static binary (musl) – a few MB, no shell, nothing to patch.
FROM scratch
COPY --from=pick /platctl-pkg /usr/local/bin/platctl-pkg
# Non-root user id (no /etc/passwd needed for a numeric id).
USER 65532:65532
ENTRYPOINT ["/usr/local/bin/platctl-pkg"]
