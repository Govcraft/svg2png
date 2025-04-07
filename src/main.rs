//! # SVG to PNG Conversion Service
//!
//! A simple Axum web service that converts SVG images to PNG format.
//! It provides an endpoint `/svg-to-png` that accepts SVG data via POST requests
//! and returns the corresponding PNG image. An optional `dpi` query parameter
//! can be used to control the output resolution. A `/health` endpoint is also
//! available for health checks.
//!
//! ## Configuration
//!
//! The service can be configured using environment variables:
//! - `RUST_LOG`: Sets the logging level (e.g., `info`, `debug`, `svg2png=trace`). Defaults to `info`.
//! - `SVG2PNG_HOST`: The host address to bind to. Defaults to `0.0.0.0`.
//! - `SVG2PNG_PORT`: The port to bind to. Defaults to `3000`.

use axum::{
    body::Bytes,
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tracing::{debug, error, info, instrument};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Environment variable name for the host address.
const HOST_ENV_VAR: &str = "SVG2PNG_HOST";
/// Environment variable name for the port number.
const PORT_ENV_VAR: &str = "SVG2PNG_PORT";
/// Environment variable name for the port number.
const DEFAULT_HOST: &str = "0.0.0.0";
/// Default port number if `SVG2PNG_PORT` is not set.
const DEFAULT_PORT: &str = "3000";
/// Default host address if `SVG2PNG_HOST` is not set.
const DPI_QUERY_PARAM: &str = "dpi";
/// HTTP Content-Type value for PNG images.
const PNG_CONTENT_TYPE: &str = "image/png";
/// Default port number if `SVG2PNG_PORT` is not set.
const DEFAULT_DPI: f32 = 96.0;

/// Query parameter name for specifying the desired output DPI.
// The `instrument` macro automatically adds logging for function entry/exit.
#[instrument(skip(body))]
/// Converts an SVG image provided in the request body to a PNG image.
///
/// Accepts an optional `dpi` query parameter to control the output resolution.
/// If `dpi` is not provided, invalid, or non-positive, it defaults to 96 DPI.
/// The SVG is scaled according to the requested DPI relative to the default 96 DPI.
///
/// The resulting PNG image includes a `pHYs` chunk indicating the physical pixel
/// dimensions based on the requested DPI.
///
/// # Arguments
///
/// * `uri` - The request URI, used to extract the optional `dpi` query parameter.
/// * `body` - The raw bytes of the SVG image data from the request body.
///
/// # Returns
///
/// * `Ok(impl IntoResponse)` - On success, returns a response containing the PNG image
///   data with a `Content-Type` header set to `image/png`.
/// * `Err((StatusCode, String))` - On failure, returns an HTTP status code and an
///   error message string. Possible errors include:
///     - `400 Bad Request`: If the request body is empty, the SVG data is invalid,
///       or the SVG dimensions result in a zero-sized image after scaling.
///     - `500 Internal Server Error`: If there's an issue creating the internal
///       pixmap or encoding the PNG data.
///
/// # Panics
///
/// This function relies on `resvg::render`, which may panic on certain SVG rendering
/// errors. Consider adding panic handling (e.g., `std::panic::catch_unwind`) if
/// robustness against potential panics is critical.
async fn svg_to_png(
    uri: Uri,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    debug!(query = uri.query().unwrap_or(""), uri = %uri, "Processing svg_to_png request");

    if body.is_empty() {
        error!("Received empty request body");
        return Err((StatusCode::BAD_REQUEST, "Request body cannot be empty".to_string()));
    }

    let mut requested_dpi = DEFAULT_DPI;

    if let Some(query) = uri.query() {
        // Iterate over query parameters using form_urlencoded.
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == DPI_QUERY_PARAM {
                // Try to parse the DPI value.
                if let Ok(dpi_val) = value.parse::<f32>() {
                    // Use the parsed value only if it's positive.
                    if dpi_val > 0.0 {
                        requested_dpi = dpi_val;
                    }
                }
                // Found the dpi key, no need to check further query parameters.
                // Note: `dpi_val` is only in scope within this `if let` block.
                debug!(%value, "Parsed DPI from query string");
                break;
            }
        }
    }

    // Note: `usvg::Options::dpi` is not used directly as its effect on scaling wasn't
    // clear from documentation at the time of writing. Manual scaling via `resvg::render`
    // transform is used instead for explicit control.
    // Explicitly set options, ensuring the default font family is set.
    let mut opt = resvg::usvg::Options::default();
    opt.font_family = "Times New Roman".to_string(); // Explicitly set default font
    debug!(options = ?opt, "Parsing SVG data with default options");
    let tree = resvg::usvg::Tree::from_data(&body, &opt).map_err(|e| {
        error!(error = %e, "Invalid SVG data received");
        (StatusCode::BAD_REQUEST, format!("Invalid SVG: {}", e))
    })?;

    // Calculate the scale factor based on the requested DPI relative to the default.
    let scale = requested_dpi / DEFAULT_DPI;

    let base_size = tree.size();
    debug!(?base_size, "Got base SVG size");
    let base_width = base_size.width();
    let base_height = base_size.height();

    // Calculate the target pixmap dimensions based on the scale factor.
    // Using `ceil()` ensures the pixmap is large enough to contain the scaled image
    // without clipping.
    let target_width = (base_width * scale).ceil() as u32;
    let target_height = (base_height * scale).ceil() as u32;
    debug!(target_width, target_height, scale, "Calculated target pixmap dimensions");

    if target_width == 0 || target_height == 0 {
        let err_msg = "SVG results in zero width or height after scaling".to_string();
        error!(%err_msg, base_width, base_height, scale);
        return Err((StatusCode::BAD_REQUEST, err_msg));
    }

    debug!(target_width, target_height, "Creating pixmap");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height).ok_or_else(|| {
        let err_msg = "Failed to create pixmap".to_string();
        error!(%err_msg, target_width, target_height);
        (StatusCode::INTERNAL_SERVER_ERROR, err_msg)
    })?;

    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    debug!(?transform, "Rendering SVG to pixmap");
    // Render the SVG tree to the pixmap using the calculated scaling transform.
    // Note: `resvg::render` can panic on certain rendering errors. Consider using
    // `std::panic::catch_unwind` if robust handling of potential panics is required.
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    debug!("SVG rendering complete");

    let png_buffer = {
        // Create a buffer to hold the resulting PNG data.
        let mut buffer = Vec::new();
        // Create a PNG encoder that will write to the buffer.
        let mut encoder = png::Encoder::new(&mut buffer, target_width, target_height);
        // Set standard PNG color type and bit depth (RGBA 8-bit).
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        // Get a writer for the image data. This must be done *before* writing
        // custom chunks like pHYs.
        debug!("Writing PNG header");
        let mut writer = encoder.write_header().map_err(|e| {
            error!(error = %e, "Failed to write PNG header");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG header: {}", e))
        })?;

        // Calculate pixels per meter for the pHYs chunk (1 inch = 0.0254 meters).
        let ppm = (requested_dpi / 0.0254).round() as u32;
        debug!(ppm, requested_dpi, "Calculated PPM for pHYs chunk");

        // Manually construct and write the pHYs chunk (physical pixel dimensions).
        // Format: 4 bytes X ppm (big-endian), 4 bytes Y ppm (big-endian), 1 byte unit specifier.
        let mut phys_data = [0u8; 9];
        phys_data[0..4].copy_from_slice(&ppm.to_be_bytes());
        phys_data[4..8].copy_from_slice(&ppm.to_be_bytes());
        phys_data[8] = 1; // Unit specifier: 1 means the unit is meters.
        debug!("Writing pHYs chunk");
        writer.write_chunk(png::chunk::pHYs, &phys_data).map_err(|e| {
            error!(error = %e, "Failed to write pHYs chunk");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write pHYs chunk: {}", e))
        })?;

        debug!("Writing PNG image data");
        // Write the actual pixel data from the rendered pixmap.
        writer.write_image_data(pixmap.data()).map_err(|e| {
            error!(error = %e, "Failed to write PNG data");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG data: {}", e))
        })?;
        // The `writer` must be dropped here to finalize the PNG stream correctly
        // before the buffer is returned.
        drop(writer);

        buffer
    };
    debug!("PNG encoding complete");

    // Note: Function exit logging is handled automatically by the `#[instrument]` macro.
    Ok((
        [(header::CONTENT_TYPE, PNG_CONTENT_TYPE)],
        png_buffer,
    ))
}

// The `instrument` macro automatically adds logging for function entry/exit.
#[instrument]

/// A simple health check endpoint.
///
/// Returns an HTTP `200 OK` status code if the service is running.
///
/// # Returns
///
/// * `StatusCode::OK` - Always returns a 200 OK status.
async fn health_check() -> StatusCode {
    StatusCode::OK
}

use anyhow::Context; // Provides the `context` method for easy error wrapping.

// Use `anyhow::Result` for convenient error handling throughout the application setup.
#[tokio::main]
/// The main entry point for the SVG to PNG conversion service.
///
/// Initializes the tracing subscriber for logging, sets up the Axum web server,
/// defines routes for health checks (`/health`) and SVG conversion (`/svg-to-png`),
/// binds to a host and port specified by environment variables (`SVG2PNG_HOST`,
/// `SVG2PNG_PORT`) or defaults (`0.0.0.0:3000`), and runs the server with
/// graceful shutdown handling for SIGINT (Ctrl+C) and SIGTERM (on Unix).
///
/// # Environment Variables
///
/// * `RUST_LOG`: Controls logging levels (e.g., `svg2png=debug,info`). Defaults to `info`.
/// * `SVG2PNG_HOST`: The host address to bind to. Defaults to `0.0.0.0`.
/// * `SVG2PNG_PORT`: The port to bind to. Defaults to `3000`.
///
/// # Returns
///
/// * `Ok(())` - If the server runs and shuts down gracefully.
/// * `Err(anyhow::Error)` - If there is an error during setup (e.g., binding the port)
///   or during server execution. Errors are wrapped with context using `anyhow`.
async fn main() -> anyhow::Result<()> {
    // Initialize the tracing subscriber setup.
    // Use `EnvFilter` to allow configuring log levels via the `RUST_LOG` environment variable.
    // Example: `RUST_LOG=svg2png=debug,tower_http=trace cargo run`
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))) // Default to "info" level if RUST_LOG is not set or invalid.
        .init();

    info!("Initializing server {} v{}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Read host and port from environment variables, falling back to defaults.
    let host = std::env::var(HOST_ENV_VAR).unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port_str = std::env::var(PORT_ENV_VAR).unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let port = port_str.parse::<u16>().context(format!("Invalid PORT value: {}", port_str))?;
    let bind_addr = format!("{}:{}", host, port);

    // Define the application routes.
    let app = Router::new()
        .route("/svg-to-png", post(svg_to_png))
        .route("/health", get(health_check));

    // Bind the TCP listener to the specified address.
    debug!("Attempting to bind to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context(format!("Failed to bind to address {}", bind_addr))?;
    let addr = listener.local_addr()?;
    info!(
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
        address = %addr,
        "Server listening"
    );
    // Set up signal handling for graceful shutdown.
    let shutdown_signal = async {
        let ctrl_c = async {
            tokio::signal::ctrl_c()
                .await
                .expect("Failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>(); // On non-Unix platforms, only Ctrl+C is monitored.

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }

        info!("Shutdown signal received, starting graceful shutdown...");
    };

    // Run the Axum server.
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal)
        .await
        .context("Axum server error")?; // Add context to potential server errors.

    info!("Server shut down gracefully.");
    Ok(())
}