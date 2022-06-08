#![allow(dead_code)]

use wasm_bindgen::prelude::*;

pub mod player;
mod bindings;
mod content;
mod media_source;
mod parser;
mod requester;
mod utils;

pub use utils::logger::Logger;
