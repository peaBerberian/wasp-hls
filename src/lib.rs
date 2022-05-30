#![allow(dead_code)]

use wasm_bindgen::prelude::*;

pub mod player;
mod bindings;
mod content;
mod parser;
mod requester;
mod source_buffer;
mod utils;

pub use utils::logger::Logger;
