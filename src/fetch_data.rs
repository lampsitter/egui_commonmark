use crate::ImageHashMap;

#[cfg(not(feature = "fetch"))]
pub fn get_image_data(path: &str, _ctx: &egui::Context, _images: ImageHashMap) -> Option<Vec<u8>> {
    get_image_data_from_file(path)
}

#[cfg(feature = "fetch")]
fn get_image_data(path: &str, ctx: &egui::Context, images: ImageHashMap) -> Option<Vec<u8>> {
    let url = url::Url::parse(path);
    if url.is_ok() {
        let ctx2 = ctx.clone();
        let path = path.to_owned();
        ehttp::fetch(ehttp::Request::get(&path), move |r| {
            if let Ok(r) = r {
                let data = r.bytes;
                if let Some(handle) = parse_image(&ctx2, &path, &data) {
                    // we only update if the image was loaded properly
                    *images.lock().unwrap().get_mut(&path).unwrap() = Some(handle);
                    ctx2.request_repaint();
                }
            }
        });

        None
    } else {
        get_image_data_from_file(path)
    }
}

fn get_image_data_from_file(url: &str) -> Option<Vec<u8>> {
    std::fs::read(url).ok()
}
