// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod books;
pub mod music;
pub mod parse;
pub mod scanner;

mod art;

pub use art::{
    ARTWORK_MAX, BACKDROP_NAMES, POSTER_NAMES, image_type, local_art, local_still, local_version, validate_image,
};
pub use scanner::Scanner;
