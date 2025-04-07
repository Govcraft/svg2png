use axum::{
    body::Bytes,
    http::{header, StatusCode, Uri},
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use tracing::{debug, error, info, instrument}; // Import tracing macros (removed warn)
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

// --- Constants ---
const HOST_ENV_VAR: &str = "HOST";
const PORT_ENV_VAR: &str = "PORT";
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_PORT: &str = "3000";
const DPI_QUERY_PARAM: &str = "dpi";
const PNG_CONTENT_TYPE: &str = "image/png";
// --- End Constants ---

// Handler to convert posted SVG to PNG, now accepting DPI query parameter via manual parsing
#[instrument(skip(body))] // Correct placement of instrument macro
async fn svg_to_png(
    uri: Uri,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Log entry details, including the query string if present
    debug!(query = uri.query().unwrap_or(""), uri = %uri, "Processing svg_to_png request"); // Log uri here instead

    // --- Manual DPI Parsing ---
    const DEFAULT_DPI: f32 = 96.0;
    let mut requested_dpi = DEFAULT_DPI; // Start with default

    if let Some(query) = uri.query() {
        // Iterate over query parameters using form_urlencoded
        for (key, value) in form_urlencoded::parse(query.as_bytes()) { // Use form_urlencoded for parsing
            if key == DPI_QUERY_PARAM { // Use constant for query param name
                // Try to parse the value as f32
                if let Ok(dpi_val) = value.parse::<f32>() {
                    // Use the parsed value if it's positive
                    if dpi_val > 0.0 {
                        requested_dpi = dpi_val;
                    }
                }
                // Found the dpi key, no need to check further params
                // Note: dpi_val is only in scope within the if-let
                debug!(%value, "Parsed DPI from query string");
                break;
            }
        }
    }
    // --- End Manual DPI Parsing ---

    // Parse the SVG data from the request body
    // Note: We are not using usvg::Options { dpi: ... } as its effect wasn't clear from docs.
    // Manual scaling via transform is used instead.
    let opt = resvg::usvg::Options::default();
    debug!(options = ?opt, "Parsing SVG data with default options");
    let tree = resvg::usvg::Tree::from_data(&body, &opt).map_err(|e| {
        error!(error = %e, "Invalid SVG data received");
        (StatusCode::BAD_REQUEST, format!("Invalid SVG: {}", e))
    })?;

    // Calculate the scale factor based on the determined DPI
    let scale = requested_dpi / DEFAULT_DPI; // DEFAULT_DPI is const 96.0

    // Get the base size of the SVG tree
    let base_size = tree.size();
    debug!(?base_size, "Got base SVG size");
    let base_width = base_size.width();
    let base_height = base_size.height();

    // Calculate the target pixmap dimensions based on the scale factor
    // Using ceil() ensures the pixmap is large enough for the scaled image
    let target_width = (base_width * scale).ceil() as u32;
    let target_height = (base_height * scale).ceil() as u32;
    debug!(target_width, target_height, scale, "Calculated target pixmap dimensions");

    // Check for zero dimensions after scaling
    if target_width == 0 || target_height == 0 {
        let err_msg = "SVG results in zero width or height after scaling".to_string();
        error!(%err_msg, base_width, base_height, scale);
        return Err((StatusCode::BAD_REQUEST, err_msg));
    }

    // Create a pixmap with the target dimensions
    debug!(target_width, target_height, "Creating pixmap");
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height).ok_or_else(|| {
        let err_msg = "Failed to create pixmap".to_string();
        error!(%err_msg, target_width, target_height);
        (StatusCode::INTERNAL_SERVER_ERROR, err_msg)
    })?;

    // Define the scaling transform
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    debug!(?transform, "Rendering SVG to pixmap");
    // Render the SVG tree to the pixmap using the scaling transform.
    // Note: resvg::render panics on error, consider catch_unwind if needed
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    debug!("SVG rendering complete");

    // --- Manual PNG Encoding with DPI ---
    let png_buffer = {
        // Create a buffer to hold the PNG data
        let mut buffer = Vec::new();
        // Create a PNG encoder targeting the buffer
        let mut encoder = png::Encoder::new(&mut buffer, target_width, target_height);
        // Set color type and bit depth (RGBA 8-bit)
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        // Get a writer for the image data *before* writing custom chunks
        debug!("Writing PNG header");
        let mut writer = encoder.write_header().map_err(|e| {
            error!(error = %e, "Failed to write PNG header");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG header: {}", e))
        })?;

        // Calculate pixels per meter (1 inch = 0.0254 meters)
        let ppm = (requested_dpi / 0.0254).round() as u32;
        debug!(ppm, requested_dpi, "Calculated PPM for pHYs chunk");

        // Manually write the pHYs chunk (physical pixel dimensions)
        // Data format: 4 bytes X ppm (big-endian), 4 bytes Y ppm (big-endian), 1 byte unit specifier (1 for meter)
        let mut phys_data = [0u8; 9];
        phys_data[0..4].copy_from_slice(&ppm.to_be_bytes());
        phys_data[4..8].copy_from_slice(&ppm.to_be_bytes());
        phys_data[8] = 1; // Unit is meters
        debug!("Writing pHYs chunk");
        writer.write_chunk(png::chunk::pHYs, &phys_data).map_err(|e| {
            error!(error = %e, "Failed to write pHYs chunk");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write pHYs chunk: {}", e))
        })?;

        debug!("Writing PNG image data");
        // Write the pixel data from the pixmap
        writer.write_image_data(pixmap.data()).map_err(|e| {
            error!(error = %e, "Failed to write PNG data");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG data: {}", e))
        })?;

        // Drop the writer to finalize the PNG stream (important!)
        drop(writer);

        // Return the buffer containing the encoded PNG
        buffer
    };
    debug!("PNG encoding complete");
    // --- End Manual PNG Encoding ---

    // Logging completion is handled by the #[instrument] macro's exit event
    // Return the PNG as a response with appropriate headers
    Ok((
        [(header::CONTENT_TYPE, PNG_CONTENT_TYPE)], // Use constant for content type
        png_buffer,
    ))
}

#[instrument] // Add instrumentation

// Simple health check handler
async fn health_check() -> StatusCode {
    StatusCode::OK
}

use anyhow::Context; // Import anyhow context for better error messages

// Main function now returns anyhow::Result for proper error handling
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing subscriber
    // Use EnvFilter to allow configuring log level via RUST_LOG env var
    // Example: RUST_LOG=svg2png=debug,tower_http=trace cargo run
    // Default level is INFO if RUST_LOG is not set.
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))) // Default to info
        .init();

    info!("Initializing server {} v{}...", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));

    // Read host and port from environment variables with defaults, using constants
    let host = std::env::var(HOST_ENV_VAR).unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let port_str = std::env::var(PORT_ENV_VAR).unwrap_or_else(|_| DEFAULT_PORT.to_string());
    let port = port_str.parse::<u16>().context(format!("Invalid PORT value: {}", port_str))?;
    let bind_addr = format!("{}:{}", host, port);

    // Build the Axum router with both routes
    let app = Router::new()
        .route("/svg-to-png", post(svg_to_png))
        .route("/health", get(health_check));

    // Start the server, using `?` and `context` for error handling
    info!("Attempting to bind to {}", bind_addr);
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .context(format!("Failed to bind to address {}", bind_addr))?;
    let addr = listener.local_addr()?;
    // Log server name, version, and listening address
    info!(
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
        address = %addr,
        "Server listening"
    );
    axum::serve(listener, app)
        .await
        .context("Axum server failed")?;

    Ok(()) // Indicate successful execution
}