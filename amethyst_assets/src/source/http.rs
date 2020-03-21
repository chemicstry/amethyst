use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};
use js_sys::Uint8Array;

#[cfg(feature = "profiler")]
use thread_profiler::profile_scope;

use amethyst_error::{format_err, Error, ResultExt};

use crate::{error, source::Source};

/// HTTP source.
///
/// Loads assets inside web worker using XmlHttpRequest.
/// Used as a default source for WASM target.
#[derive(Debug)]
pub struct HTTP {
    loc: PathBuf,
}

impl HTTP {
    /// Creates a new http source.
    pub fn new<P>(loc: P) -> Self
    where
        P: Into<PathBuf>,
    {
        HTTP { loc: loc.into() }
    }

    fn path(&self, s_path: &str) -> PathBuf {
        let mut path = self.loc.clone();
        path.extend(Path::new(s_path).iter());

        path
    }
}

impl Source for HTTP {
    fn modified(&self, path: &str) -> Result<u64, Error> {
        #[cfg(feature = "profiler")]
        profile_scope!("http_modified_asset");
        
        // Unimplemented. Maybe possible to tie into webpack hot module reloading?
        Ok(0)
    }

    fn load(&self, path: &str) -> Result<Vec<u8>, Error> {
        #[cfg(feature = "profiler")]
        profile_scope!("http_load_asset");

        let path = self.path(path);
        let path_str = path.to_str()
            .ok_or_else(|| format_err!("Path contains non-unicode characters {:?}", path))
            .with_context(|_| error::Error::Source)?;

        let xhr = web_sys::XmlHttpRequest::new()
            .map_err(|_| format_err!("Failed to construct XmlHttpRequest"))
            .with_context(|_| error::Error::Source)?;

        // Synchronous GET request. Should only be run in web worker.
        xhr.open_with_async("GET", path_str, false);
        xhr.set_response_type(web_sys::XmlHttpRequestResponseType::Arraybuffer);

        // We block here and wait for http fetch to complete
        xhr.send()
            .map_err(|_| format_err!("XmlHttpRequest send failed"))
            .with_context(|_| error::Error::Source)?;

        // Status returns a result but according to javascript spec it should never return error.
        // Returns 0 is request was not completed.
        let status = xhr.status().unwrap();
        if status != 200 {
            let msg = xhr.status_text().unwrap_or("".to_string());
            return Err(format_err!("XmlHttpRequest failed with code {}. Error: {}", status, msg))
                .with_context(|_| error::Error::Source)
        }

        let resp = xhr.response().unwrap();

        // Convert javascript ArrayBuffer into Vec<u8>
        let arr = Uint8Array::new(&resp);
        let mut v = vec![0; arr.length() as usize];
        arr.copy_to(&mut v);

        Ok(v)
    }
}
