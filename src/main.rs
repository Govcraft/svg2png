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

    // Parse the SVG data from the request body
    // Note: We are not using usvg::Options { dpi: ... } as its effect wasn't clear from docs.
    // Manual scaling via transform is used instead.
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&body, &opt)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid SVG: {}", e)))?;

    // Calculate the scale factor based on the determined DPI
    let scale = requested_dpi / DEFAULT_DPI; // DEFAULT_DPI is const 96.0

    // Get the base size of the SVG tree
    let base_size = tree.size();
    let base_width = base_size.width();
    let base_height = base_size.height();

    // Calculate the target pixmap dimensions based on the scale factor
    // Using ceil() ensures the pixmap is large enough for the scaled image
    let target_width = (base_width * scale).ceil() as u32;
    let target_height = (base_height * scale).ceil() as u32;

    // Check for zero dimensions after scaling
    if target_width == 0 || target_height == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "SVG results in zero width or height after scaling".to_string(),
        ));
    }

    // Create a pixmap with the target dimensions
    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create pixmap".to_string()))?;

    // Define the scaling transform
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);

    // Render the SVG tree to the pixmap using the scaling transform.
    resvg::render(&tree, transform, &mut pixmap.as_mut());

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
        let mut writer = encoder.write_header()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG header: {}", e)))?;

        // Calculate pixels per meter (1 inch = 0.0254 meters)
        let ppm = (requested_dpi / 0.0254).round() as u32;

        // Manually write the pHYs chunk (physical pixel dimensions)
        // Data format: 4 bytes X ppm (big-endian), 4 bytes Y ppm (big-endian), 1 byte unit specifier (1 for meter)
        let mut phys_data = [0u8; 9];
        phys_data[0..4].copy_from_slice(&ppm.to_be_bytes());
        phys_data[4..8].copy_from_slice(&ppm.to_be_bytes());
        phys_data[8] = 1; // Unit is meters
        writer.write_chunk(png::chunk::pHYs, &phys_data)
             .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write pHYs chunk: {}", e)))?;

        // Write the pixel data from the pixmap
        writer.write_image_data(pixmap.data())
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write PNG data: {}", e)))?;

        // Drop the writer to finalize the PNG stream (important!)
        drop(writer);

        // Return the buffer containing the encoded PNG
        buffer
    };
    // --- End Manual PNG Encoding ---

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