#[cfg(not(feature = "fetch"))]
pub fn get_image_data(uri: &str, on_done: impl 'static + Send + FnOnce(Result<Vec<u8>, String>)) {
    get_image_data_from_file(uri, on_done)
}

#[cfg(feature = "fetch")]
pub fn get_image_data(uri: &str, on_done: impl 'static + Send + FnOnce(Result<Vec<u8>, String>)) {
    let url = url::Url::parse(uri);
    if url.is_ok() {
        let uri = uri.to_owned();
        ehttp::fetch(ehttp::Request::get(&uri), move |result| match result {
            Ok(response) => {
                on_done(Ok(response.bytes));
            }
            Err(err) => {
                on_done(Err(err));
            }
        });
    } else {
        get_image_data_from_file(uri, on_done)
    }
}

fn get_image_data_from_file(
    path: &str,
    on_done: impl 'static + Send + FnOnce(Result<Vec<u8>, String>),
) {
    on_done(std::fs::read(path).map_err(|err| err.to_string()));
}
