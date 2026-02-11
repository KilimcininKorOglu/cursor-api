# Define build arguments
ARG TARGETARCH
ARG BUILD_PREVIEW=false
ARG BUILD_COMPAT=false

# ==================== Build Stage ====================
FROM --platform=linux/${TARGETARCH} rustlang/rust:nightly-trixie-slim AS builder

ARG TARGETARCH
ARG BUILD_PREVIEW
ARG BUILD_COMPAT

WORKDIR /build

# Install build dependencies and Rust musl toolchain
RUN apt-get update && \
    apt-get install -y --no-install-recommends gcc nodejs npm lld musl-tools && \
    rm -rf /var/lib/apt/lists/* && \
    case "$TARGETARCH" in \
        amd64) rustup target add x86_64-unknown-linux-musl ;; \
        arm64) rustup target add aarch64-unknown-linux-musl ;; \
        *) echo "Unsupported architecture for rustup: $TARGETARCH" && exit 1 ;; \
    esac

COPY . .

# Set compilation parameters based on build options and build the project
RUN \
    # Set compilation target and optimized CPU model based on architecture
    case "$TARGETARCH" in \
        amd64) \
            TARGET_TRIPLE="x86_64-unknown-linux-musl"; \
            TARGET_CPU="x86-64-v3" ;; \
        arm64) \
            TARGET_TRIPLE="aarch64-unknown-linux-musl"; \
            TARGET_CPU="neoverse-n1" ;; \
        *) echo "Unsupported architecture: $TARGETARCH" && exit 1 ;; \
    esac && \
    \
    # Combine cargo features
    FEATURES="" && \
    if [ "$BUILD_PREVIEW" = "true" ]; then FEATURES="$FEATURES __preview_locked"; fi && \
    if [ "$BUILD_COMPAT" = "true" ]; then FEATURES="$FEATURES __compat"; fi && \
    FEATURES=$(echo "$FEATURES" | xargs) && \
    \
    # Prepare RUSTFLAGS, remove specific CPU optimization in compat mode for better compatibility
    RUSTFLAGS_BASE="-C link-arg=-s -C link-arg=-fuse-ld=lld -C target-feature=+crt-static -A unused" && \
    if [ "$BUILD_COMPAT" = "true" ]; then \
        export RUSTFLAGS="$RUSTFLAGS_BASE"; \
    else \
        export RUSTFLAGS="$RUSTFLAGS_BASE -C target-cpu=$TARGET_CPU"; \
    fi && \
    \
    # Execute build
    # -C link-arg=-s: Remove symbol table to reduce size
    # -C target-feature=+crt-static: Statically link C runtime
    # -C target-cpu: Optimize for specific CPU
    # -A unused: Allow unused code
    if [ -n "$FEATURES" ]; then \
        cargo build --bin cursor-api --release --target=$TARGET_TRIPLE --features "$FEATURES"; \
    else \
        cargo build --bin cursor-api --release --target=$TARGET_TRIPLE; \
    fi && \
    \
    mkdir -p /app && \
    cp target/$TARGET_TRIPLE/release/cursor-api /app/

# ==================== Runtime Stage ====================
FROM scratch

# Copy binary from build stage and set ownership to non-root user
COPY --chown=1001:1001 --chmod=0700 --from=builder /app /app

WORKDIR /app

ENV PORT=3000
EXPOSE ${PORT}

# Run as non-root user for enhanced security
USER 1001

ENTRYPOINT ["/app/cursor-api"]
