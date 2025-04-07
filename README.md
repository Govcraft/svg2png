# svg2png Microservice

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
<!-- Add other relevant badges here, e.g., build status, latest release -->

A performant Rust microservice using Axum and `resvg` to easily convert SVG images to PNG format with adjustable DPI via a simple HTTP API.

## Features

*   **High-Performance Conversion:** Leverages Rust and the `resvg` library for efficient SVG rendering.
*   **Simple HTTP API:** Provides a straightforward `/svg-to-png` endpoint for conversion.
*   **Adjustable DPI:** Control the output resolution using the `dpi` query parameter.
*   **Health Check:** Includes a `/health` endpoint for monitoring service status.
*   **Containerized:** Official Docker images available on GitHub Container Registry (GHCR).
*   **Cross-Platform Binaries:** Pre-compiled binaries available for Linux, macOS (x86_64, aarch64), and Windows via GitHub Releases.

## Getting Started

There are several ways to run the `svg2png` service:

### 1. Using Docker (Recommended)

The easiest way to run the service is using the official Docker image from GHCR.

```bash
# Pull the latest image
docker pull ghcr.io/govcraft/svg2png:latest

# Run the container, mapping port 3000
docker run -d -p 3000:3000 --name svg2png ghcr.io/govcraft/svg2png:latest

# Run with a specific version tag (e.g., v0.1.0)
# docker run -d -p 3000:3000 --name svg2png ghcr.io/govcraft/svg2png:0.1.0

# Run with custom port and host binding (optional)
# docker run -d -p 8080:8080 \
#   -e SVG2PNG_PORT=8080 \
#   -e SVG2PNG_HOST=0.0.0.0 \
#   --name svg2png ghcr.io/govcraft/svg2png:latest
```

The service will be available at `http://localhost:3000` (or the custom port you specified).

### 2. Using Pre-built Binaries

You can download pre-compiled binaries for your operating system from the [GitHub Releases page](https://github.com/Govcraft/svg2png/releases).

1.  Download the appropriate binary for your system (e.g., `svg2png-linux-x86_64`, `svg2png-macos-aarch64`, `svg2png-windows-x86_64.exe`).
2.  Make the binary executable (on Linux/macOS): `chmod +x ./svg2png-linux-x86_64`
3.  Run the binary: `./svg2png-linux-x86_64`

The service will start using the default configuration (port 3000). You can configure it using environment variables (see [Configuration](#configuration)).

### 3. Building from Source

Ensure you have a recent Rust toolchain installed.

1.  Clone the repository:
    ```bash
    git clone https://github.com/Govcraft/svg2png.git
    cd svg2png
    ```
2.  Build the release binary:
    ```bash
    cargo build --release
    ```
3.  Run the compiled binary:
    ```bash
    ./target/release/svg2png
    ```

## API Usage

### Convert SVG to PNG

*   **Endpoint:** `/svg-to-png`
*   **Method:** `POST`
*   **Request Body:** Raw SVG data (`Content-Type: image/svg+xml` or other, though the service primarily cares about the content being valid SVG).
*   **Query Parameters:**
    *   `dpi` (optional): The desired output resolution in Dots Per Inch. Must be a positive number. Defaults to `96.0` if not provided or invalid. The SVG is scaled relative to this default DPI.
*   **Success Response:**
    *   **Status Code:** `200 OK`
    *   **Content-Type:** `image/png`
    *   **Body:** Raw PNG image data. The PNG includes a `pHYs` chunk indicating the physical pixel dimensions based on the requested DPI.
*   **Error Responses:**
    *   `400 Bad Request`: If the request body is empty, the SVG data is invalid, or the resulting image dimensions are zero after scaling.
    *   `500 Internal Server Error`: If there's an internal issue creating the image buffer or encoding the PNG.

**Example using `curl`:**

```bash
# Basic conversion (defaults to 96 DPI)
curl -X POST --data-binary @your_image.svg http://localhost:3000/svg-to-png -o output.png

# Conversion with custom DPI (e.g., 300 DPI)
curl -X POST --data-binary @your_image.svg "http://localhost:3000/svg-to-png?dpi=300" -o output_300dpi.png
```

*(Replace `your_image.svg` with the path to your SVG file and `localhost:3000` with the correct host/port if not using defaults)*

### Health Check

*   **Endpoint:** `/health`
*   **Method:** `GET`
*   **Success Response:**
    *   **Status Code:** `200 OK`
    *   **Body:** Empty

**Example using `curl`:**

```bash
curl http://localhost:3000/health
# Expected output: (No body, just HTTP 200 status)

# Check status code
curl -o /dev/null -s -w "%{http_code}\n" http://localhost:3000/health
# Expected output: 200
```

## Configuration

The service can be configured using the following environment variables:

| Variable        | Description                                      | Default   |
| :-------------- | :----------------------------------------------- | :-------- |
| `SVG2PNG_HOST`  | The host address the server binds to.            | `0.0.0.0` |
| `SVG2PNG_PORT`  | The port the server listens on.                  | `3000`    |
| `RUST_LOG`      | Controls logging level and verbosity.            | `info`    |
|                 | (e.g., `debug`, `svg2png=trace`, `warn`)         |           |

## Building

To build the project locally, ensure you have Rust installed and run:

```bash
cargo build
# For an optimized release build:
cargo build --release
```

## Contributing

Contributions are welcome! Please refer to the contribution guidelines (if available) or open an issue/pull request on GitHub.

## License

This project is licensed under the [MIT License](LICENSE).