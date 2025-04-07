use axum::{
    body::Bytes,
    http::{header, StatusCode, Uri}, // Changed to Uri for manual query parsing
    response::IntoResponse,
    routing::post,
    Router,
};

// Handler to convert posted SVG to PNG, now accepting DPI query parameter via manual parsing
async fn svg_to_png(
    uri: Uri, // Accept the full URI to parse the query string
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // --- Manual DPI Parsing ---
    const DEFAULT_DPI: f32 = 96.0;
    let mut requested_dpi = DEFAULT_DPI; // Start with default

    if let Some(query) = uri.query() {
        // Iterate over query parameters using form_urlencoded
        for (key, value) in form_urlencoded::parse(query.as_bytes()) {
            if key == "dpi" {
                // Try to parse the value as f32
                if let Ok(dpi_val) = value.parse::<f32>() {
                    // Use the parsed value if it's positive
                    if dpi_val > 0.0 {
                        requested_dpi = dpi_val;
                    }
                }
                // Found the dpi key, no need to check further params
                break;
            }
        }
    }
    // --- End Manual DPI Parsing ---

    // Create usvg options and set the DPI based on the parsed query parameter
    let mut opt = resvg::usvg::Options::default();
    opt.dpi = requested_dpi; // Set the DPI for usvg to handle unit conversions

    // Parse the SVG data using the specified options
    let tree = resvg::usvg::Tree::from_data(&body, &opt)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid SVG: {}", e)))?;

    // Get the size of the tree (which should be affected by the DPI option)
    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;

    // Check for zero dimensions (could happen with empty SVGs or extreme DPI)
    if width == 0 || height == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "SVG results in zero width or height after DPI conversion".to_string(),
        ));
    }

    // Create a pixmap with the calculated dimensions
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create pixmap".to_string()))?;

    // Render using the identity transform, as scaling is handled by usvg options
    let transform = resvg::tiny_skia::Transform::identity();
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // Encode the pixmap into a PNG byte buffer
    let png_buffer = pixmap
        .encode_png()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to encode PNG: {}", e)))?;

    // Return the PNG as a response with appropriate headers
    Ok((
        [(header::CONTENT_TYPE, "image/png")],
        png_buffer,
    ))
}

// Main function to set up the Axum server
#[tokio::main]
async fn main() {
    // Build the Axum router
    let app = Router::new()
        .route("/svg-to-png", post(svg_to_png));

    // Start the server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running on http://0.0.0.0:3000");
    axum::serve(listener, app).await.unwrap();
}