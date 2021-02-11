//! ffs - the Friendly Filesystem
//!
//! Wraps std::fs (or optionally, tokio::fs) in functions with identical
//! signatures such that error messages include extra context, in particular
//! the path used in the function call.
//!
//! This helps with "file not found" errors. Without ffs, the error would be:
//!
//!     Error: No such file or directory (os error 2)
//!
//! and with ffs, the error becomes:
//!
//!     @TODO

use std::path::Path;

mod io_error;

pub use crate::io_error::{IoError, IoResult};

macro_rules! impl_ffs {
    ( $( fn $name:ident (path $(, $arg:ident : $arg_ty:ty)* ) -> $output:ty ; )* ) => {

        $(
            pub async fn $name<P: Clone + AsRef<Path>>(path: P $(, $arg : $arg_ty)*) -> IoResult<$output> {
                let err_path = path.as_ref().to_owned();
                let mapper = move |e| IoError::new(e, Some(err_path));
                #[cfg(feature = "tokio")]
                return tokio::fs::$name(path $(, $arg)*).await.map_err(mapper);

                #[cfg(not(feature = "tokio"))]
                return std::fs::$name(path $(, $arg)*).map_err(mapper);
            }
        )*
    };
}

impl_ffs! {
    fn read(path) -> Vec<u8>;
    fn read_to_string(path) -> String;
    fn write(path, data: &[u8]) -> ();
    fn create_dir_all(path) -> ();
}
