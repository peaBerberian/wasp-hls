#![allow(dead_code)]

use wasm_bindgen::prelude::*;

pub mod frontend;
mod adaptive;
mod bindings;
mod buffers;
mod content;
mod parser;
mod requester;
mod segment_selector;
mod utils;

pub use utils::logger::Logger;
