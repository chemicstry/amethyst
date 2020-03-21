use std::path::{Path, PathBuf};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};
use js_sys::Uint8Array;

use async_trait::async_trait;

#[cfg(feature = "profiler")]
use thread_profiler::profile_scope;

use amethyst_error::{format_err, Error, ResultExt};

use crate::{error, source::Source};

/// HTTP source.
///
/// Please note that there is a default directory source
/// inside the `Loader`, which is automatically used when you call
/// `load`. In case you want another, second, directory for assets,
/// you can instantiate one yourself, too. Please use `Loader::load_from` then.
#[derive(Debug)]
pub struct HTTP {
    loc: PathBuf,
}

impl HTTP {
    /// Creates a new http storage.
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

#[async_trait(?Send)]
impl Source for HTTP {
    fn modified(&self, path: &str) -> Result<u64, Error> {
        #[cfg(feature = "profiler")]
        profile_scope!("http_modified_asset");
        
        // Unimplemented. Maybe possible to tie into webpack hot module reloading?
        Ok(0)
    }

    async fn load(&self, path: &str) -> Result<Vec<u8>, Error> {
        #[cfg(feature = "profiler")]
        profile_scope!("http_load_asset");

        let path = self.path(path);
        let path_str = path.to_str().ok_or_else(|| error::Error::Source)?;

        let mut opts = RequestInit::new();
        opts.method("GET");
        opts.mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(path_str, &opts)
            .map_err(|_| error::Error::Source)?;

        let window = web_sys::window().unwrap();
        let resp_value = JsFuture::from(window.fetch_with_request(&request)).await.map_err(|_| error::Error::Source)?;

        // `resp_value` is a `Response` object.
        assert!(resp_value.is_instance_of::<Response>());
        let resp: Response = resp_value.dyn_into().unwrap();

        // Convert this other `Promise` into a rust `Future`.
        let arr = JsFuture::from(resp.array_buffer().map_err(|_| error::Error::Source)?).await.map_err(|_| error::Error::Source)?;

        // Convert array buffer into vec
        let arr = Uint8Array::new(&arr);
        let mut v = vec![0; arr.length() as usize];
        arr.copy_to(&mut v);

        Ok(v)
    }
}
