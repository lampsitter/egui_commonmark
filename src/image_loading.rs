use egui::ColorImage;

pub fn load_image(url: &str, data: &[u8]) -> Result<ColorImage, String> {
    if url.ends_with(".svg") {
        try_render_svg(data)
    } else {
        try_load_image(data).map_err(|err| err.to_string())
    }
}

fn try_load_image(data: &[u8]) -> image::ImageResult<ColorImage> {
    let image = image::load_from_memory(data)?;
    let image_buffer = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image_buffer.as_flat_samples();

    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

#[cfg(not(feature = "svg"))]
fn try_render_svg(_data: &[u8]) -> Result<ColorImage, String> {
    Err("SVG support not enabled".to_owned())
}

#[cfg(feature = "svg")]
fn try_render_svg(data: &[u8]) -> Result<ColorImage, String> {
    use resvg::tiny_skia;
    use usvg::{TreeParsing, TreeTextToPath};

    let tree = {
        let options = usvg::Options::default();
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();

        let mut tree = usvg::Tree::from_data(data, &options).map_err(|err| err.to_string())?;
        tree.convert_text(&fontdb);
        resvg::Tree::from_usvg(&tree)
    };

    let size = tree.size.to_int_size();

    let (w, h) = (size.width(), size.height());
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("Failed to create {w}x{h} SVG image"))?;
    tree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());

    Ok(ColorImage::from_rgba_unmultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        &pixmap.take(),
    ))
}
