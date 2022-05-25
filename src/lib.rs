#![allow(dead_code)]

use wasm_bindgen::prelude::*;

pub mod player;
mod content;
mod js_functions;
mod parser;
mod requester;
mod source_buffer;
mod utils;

pub use utils::logger::Logger;
