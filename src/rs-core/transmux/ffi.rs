//! Standalone test/benchmark ABI, compiled only with `transmux-test-api`.
//!
//! Production playback and probes own ordinary Rust Transmuxer values. They
//! never expose these handles. The separate test artifact permits isolated
//! media tests and includes the same input/output copies in measurements.

use super::{Options, Output, Transmuxer, MAX_INPUT_BYTES};

/// wasm32 layout (little endian), 40 bytes:
/// 0: output pointer, 4: length, 8: start f64, 16: end f64,
/// 24: timescale u32, 28: status (0 success, 1 empty, error code + 1).
/// 32: error text pointer, 36: error text length. Text has static Rust storage.
#[repr(C)]
#[derive(Default)]
pub struct Descriptor {
    pointer: u32,
    length: u32,
    start: f64,
    end: f64,
    timescale: u32,
    status: u32,
    error_pointer: u32,
    error_length: u32,
}

#[derive(Default)]
pub struct Session {
    muxer: Transmuxer,
    output: Option<Output>,
    descriptor: Descriptor,
}

#[unsafe(no_mangle)]
pub extern "C" fn wasp_transmuxer_new(keep_original: u32) -> *mut Session {
    Box::into_raw(Box::new(Session {
        muxer: Transmuxer {
            keep_original_timestamps: keep_original != 0,
            ..Default::default()
        },
        output: None,
        descriptor: Descriptor::default(),
    }))
}

/// # Safety
/// `session` must be a live handle from `wasp_transmuxer_new`, used once only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wasp_transmuxer_free(session: *mut Session) {
    if !session.is_null() {
        drop(unsafe { Box::from_raw(session) });
    }
}

/// # Safety
/// `session` must be live and exclusively accessed during this synchronous call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wasp_transmuxer_release(session: *mut Session) {
    if let Some(session) = unsafe { session.as_mut() } {
        session.output = None;
        session.descriptor = Descriptor::default();
    }
}

/// # Safety
/// `session` must be a live, exclusively accessed handle. `input` must point to
/// `length` initialized bytes in a separate allocation valid throughout the call.
/// The returned descriptor belongs to the session and is valid until its next
/// run/release/free call. Read memory through fresh JS views after calling us:
/// allocations here may have grown WASM memory and detached earlier views.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wasp_transmuxer_run(
    session: *mut Session,
    input: *const u8,
    length: usize,
    reset: u32,
    has_seed: u32,
    seed: f64,
    timescale: u32,
) -> *const Descriptor {
    let Some(session) = (unsafe { session.as_mut() }) else {
        return std::ptr::null();
    };
    session.output = None;
    session.descriptor = Descriptor::default();
    let result = if input.is_null() || length > MAX_INPUT_BYTES {
        Err(super::Error::Limit)
    } else {
        let bytes = unsafe { std::slice::from_raw_parts(input, length) };
        session.muxer.transmux(
            bytes,
            Options {
                reset: reset != 0,
                seed: (has_seed != 0).then_some((seed, timescale)),
            },
        )
    };
    match result {
        Ok(Some(output)) => {
            session.descriptor = Descriptor {
                pointer: output.data.as_ptr() as usize as u32,
                length: output.data.len() as u32,
                start: output.timing.start as f64,
                end: output.timing.end as f64,
                timescale: output.timing.timescale,
                status: 0,
                error_pointer: 0,
                error_length: 0,
            };
            session.output = Some(output);
        }
        Ok(None) => session.descriptor.status = 1,
        Err(error) => {
            let description = error.description();
            session.descriptor.status = error as u32 + 1;
            session.descriptor.error_pointer = description.as_ptr() as usize as u32;
            session.descriptor.error_length = description.len() as u32;
        }
    }
    &session.descriptor
}
