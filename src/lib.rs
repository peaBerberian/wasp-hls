#![allow(dead_code)]

use wasm_bindgen::prelude::*;

pub mod dispatcher;
mod adaptive;
mod bindings;
mod content_tracker;
mod media_element;
mod parser;
mod requester;
mod segment_selector;
mod utils;

pub use utils::logger::Logger;
