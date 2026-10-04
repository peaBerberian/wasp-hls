#![allow(dead_code)]

use std::mem;

mod adaptive;
mod bindings;
pub mod dispatcher;
mod media_element;
mod parser;
mod playlist_store;
mod requester;
mod segment_selector;
mod transmux;
mod utils;

#[unsafe(no_mangle)]
pub extern "C" fn wasp_malloc(len: usize) -> *mut u8 {
    let mut bytes = Vec::<u8>::with_capacity(len);
    let ptr = bytes.as_mut_ptr();
    mem::forget(bytes);
    ptr
}

#[unsafe(no_mangle)]
/// # Safety
///
/// `ptr` must have been returned by `wasp_malloc` with the same `len`.
pub unsafe extern "C" fn wasp_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        unsafe {
            drop(Vec::from_raw_parts(ptr, len, len));
        }
    }
}
