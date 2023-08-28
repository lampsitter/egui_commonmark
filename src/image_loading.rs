use egui::ColorImage;

pub fn load_image(data: &[u8]) -> Option<ColorImage> {
    try_load_image(data).ok().or_else(|| try_render_svg(data))
}

fn try_load_image(data: &[u8]) -> image::ImageResult<ColorImage> {
    let image = image::load_from_memory(data)?;
    let image_buffer = image.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let pixels = image_buffer.as_flat_samples();

    Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

#[cfg(not(feature = "svg"))]
fn try_render_svg(_data: &[u8]) -> Option<ColorImage> {
    None
}

#[cfg(feature = "svg")]
fn try_render_svg(data: &[u8]) -> Option<ColorImage> {
    use resvg::tiny_skia;
    use usvg::{TreeParsing, TreeTextToPath};

    let tree = {
        let options = usvg::Options::default();
        let mut fontdb = usvg::fontdb::Database::new();
        fontdb.load_system_fonts();

        let mut tree = usvg::Tree::from_data(data, &options).ok()?;
        tree.convert_text(&fontdb);
        resvg::Tree::from_usvg(&tree)
    };

    let size = tree.size.to_int_size();

    let mut pixmap = tiny_skia::Pixmap::new(size.width(), size.height())?;
    tree.render(tiny_skia::Transform::default(), &mut pixmap.as_mut());

    Some(ColorImage::from_rgba_unmultiplied(
        [pixmap.width() as usize, pixmap.height() as usize],
        &pixmap.take(),
    ))
}
