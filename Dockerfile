# ---- Builder Stage ----
# Use an official Rust image based on Debian Bookworm for better library compatibility
FROM rust:1-bookworm AS builder

# Install necessary build dependencies for static MUSL build and resvg
# - musl-tools: Required for linking against MUSL libc
# - pkg-config, libfontconfig1-dev: Common dependencies for resvg/font handling
RUN apt-get update && \
    apt-get install -y --no-install-recommends musl-tools libfontconfig1-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

# Add the MUSL target for static linking
RUN rustup target add x86_64-unknown-linux-musl

# Set the working directory
WORKDIR /app

# Create a dummy main.rs to cache dependency builds
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Copy manifests first to leverage Docker layer caching for dependencies
COPY Cargo.toml Cargo.lock ./

# Build only dependencies using the MUSL target
# This step is cached if Cargo.toml/Cargo.lock haven't changed
RUN cargo build --release --locked --target x86_64-unknown-linux-musl

# Copy the actual source code
COPY src ./src

# Build the final application binary, statically linked
# Ensure the dummy main.rs is removed/overwritten by the COPY above
# Using touch ensures the timestamp changes, invalidating the cache for this layer
RUN touch src/main.rs && \
    cargo build --release --locked --target x86_64-unknown-linux-musl

# ---- Final Stage ----
# Use a minimal scratch image
# Use a minimal Debian image that includes fontconfig support
FROM debian:bookworm-slim

# Set the working directory
WORKDIR /app

# Install fontconfig and the Microsoft Core Fonts (includes Times New Roman)
# Accept the EULA non-interactively
# Clean up apt cache afterwards
# Add contrib component to sources and update
RUN echo "deb http://deb.debian.org/debian bookworm contrib" > /etc/apt/sources.list.d/contrib.list && \
    apt-get update && \
    echo ttf-mscorefonts-installer msttcorefonts/accepted-mscorefonts-eula select true | debconf-set-selections && \
    apt-get install -y --no-install-recommends fontconfig ttf-mscorefonts-installer fonts-liberation2 && \
    apt-get clean && \
    fc-cache -fv && \
    rm -rf /var/lib/apt/lists/*

# Ensure font directories and cache are readable by all users
RUN chmod -R a+rX /usr/share/fonts/ /var/cache/fontconfig/

# Create a non-root user and group
RUN groupadd --gid 1001 appgroup && \
    useradd --uid 1001 --gid 1001 --shell /bin/bash --create-home appuser

# Copy the statically linked binary from the builder stage
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/svg2png .

# Change ownership of the app directory and binary
RUN chown -R appuser:appgroup /app

# Switch to the non-root user
USER appuser
# Build font cache as the appuser to ensure it's available at runtime
RUN fc-cache -fv

# Expose the default port the application listens on
EXPOSE 3000

# Set default environment variables (can be overridden at runtime)
ENV SVG2PNG_HOST=0.0.0.0
ENV SVG2PNG_PORT=3000
ENV RUST_LOG=info

# Command to run the application binary
# Command to run the application binary
CMD ["./svg2png"]