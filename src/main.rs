use axum::{
    body::Bytes,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};

// Handler to convert posted SVG to PNG
async fn svg_to_png(body: Bytes) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Parse the SVG data from the request body
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&body, &opt)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid SVG: {}", e)))?;

    // Get the size of the SVG tree. The `size` field holds width and height.
    // Convert width/height to u32 for Pixmap::new. Using ceil() ensures the pixmap is large enough.
    // Added checks for zero dimensions to prevent panic in Pixmap::new.
    let size = tree.size();
    let width = size.width().ceil() as u32;
    let height = size.height().ceil() as u32;

    if width == 0 || height == 0 {
        return Err((StatusCode::BAD_REQUEST, "SVG has zero width or height".to_string()));
    }

    // Create a pixmap with the SVG's dimensions
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, "Failed to create pixmap".to_string()))?;

    // Define the identity transform (no scaling or translation needed as pixmap matches SVG size)
    let transform = resvg::tiny_skia::Transform::identity();

    // Render the SVG tree to the pixmap using the identity transform.
    // Note: resvg::render returns () and panics on error. Consider adding catch_unwind for robustness.
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