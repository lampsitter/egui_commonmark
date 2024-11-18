use egui::load::{Bytes, BytesLoadResult, BytesLoader, BytesPoll, LoadError};
use egui::mutex::Mutex;

use std::collections::HashMap;
use std::sync::Arc;
use std::task::Poll;

pub fn install_loader(ctx: &egui::Context) {
    if !ctx.is_loader_installed(DataUrlLoader::ID) {
        ctx.add_bytes_loader(std::sync::Arc::new(DataUrlLoader::default()));
    }
}

#[derive(Clone)]
struct Data {
    bytes: Arc<[u8]>,
    mime: Option<String>,
}

type Entry = Poll<Result<Data, String>>;

#[derive(Default)]
pub struct DataUrlLoader {
    cache: Arc<Mutex<HashMap<String, Entry>>>,
}

impl DataUrlLoader {
    pub const ID: &'static str = egui::generate_loader_id!(DataUrlLoader);
}

impl BytesLoader for DataUrlLoader {
    fn id(&self) -> &str {
        Self::ID
    }

    fn load(&self, ctx: &egui::Context, uri: &str) -> BytesLoadResult {
        if data_url::DataUrl::process(uri).is_err() {
            return Err(LoadError::NotSupported);
        };

        let mut cache = self.cache.lock();
        if let Some(entry) = cache.get(uri).cloned() {
            match entry {
                Poll::Ready(Ok(file)) => Ok(BytesPoll::Ready {
                    size: None,
                    bytes: Bytes::Shared(file.bytes),
                    mime: file.mime,
                }),
                Poll::Ready(Err(err)) => Err(LoadError::Loading(err)),
                Poll::Pending => Ok(BytesPoll::Pending { size: None }),
            }
        } else {
            cache.insert(uri.to_owned(), Poll::Pending);
            drop(cache);

            let cache = self.cache.clone();
            let uri = uri.to_owned();
            let ctx = ctx.clone();

            std::thread::Builder::new()
                .name("DataUrlLoader".to_owned())
                .spawn(move || {
                    // Must unfortuntely do the process step again
                    let url = data_url::DataUrl::process(&uri);
                    match url {
                        Ok(url) => {
                            let result = url
                                .decode_to_vec()
                                .map(|(decoded, _)| {
                                    let mime = url.mime_type().to_string();
                                    let mime = if mime.is_empty() { None } else { Some(mime) };

                                    Data {
                                        bytes: decoded.into(),
                                        mime,
                                    }
                                })
                                .map_err(|e| e.to_string());
                            cache.lock().insert(uri, Poll::Ready(result));
                        }
                        Err(e) => {
                            cache.lock().insert(uri, Poll::Ready(Err(e.to_string())));
                        }
                    }

                    ctx.request_repaint();
                })
                .expect("could not spawn thread");

            Ok(BytesPoll::Pending { size: None })
        }
    }

    fn forget(&self, uri: &str) {
        let _ = self.cache.lock().remove(uri);
    }

    fn forget_all(&self) {
        self.cache.lock().clear();
    }

    fn byte_size(&self) -> usize {
        self.cache
            .lock()
            .values()
            .map(|entry| match entry {
                Poll::Ready(Ok(file)) => {
                    file.bytes.len() + file.mime.as_ref().map_or(0, |m| m.len())
                }
                Poll::Ready(Err(err)) => err.len(),
                _ => 0,
            })
            .sum()
    }
}
