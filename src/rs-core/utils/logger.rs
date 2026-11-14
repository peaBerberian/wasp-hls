use std::sync::atomic::{AtomicU8, Ordering};

#[cfg(target_arch = "wasm32")]
use crate::bindings::jsLog;

#[cfg(target_arch = "wasm32")]
use crate::bindings::LogLevel;

#[cfg(target_arch = "wasm32")]
pub(crate) fn emit_log(level: LogLevel, text: &str) {
    jsLog(level, text);
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn emit_log<T>(_level: T, _text: &str) {}

pub static MAX_LOG_LEVEL: AtomicU8 = AtomicU8::new(LoggerLevel::None as u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoggerLevel {
    None = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

impl LoggerLevel {
    pub(crate) fn from_u32(level: u32) -> Self {
        match level {
            1 => LoggerLevel::Error,
            2 => LoggerLevel::Warn,
            3 => LoggerLevel::Info,
            4 => LoggerLevel::Debug,
            _ => LoggerLevel::None,
        }
    }
}

pub(crate) fn set_log_level(level: LoggerLevel) {
    MAX_LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

macro_rules! log_debug {
    (lazy: $func:expr) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Debug as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Debug, &($func)());
        }
    };
    ($($arg:tt)*) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Debug as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Debug, &format!($($arg)*));
        }
    };
}
pub(crate) use log_debug;

macro_rules! log_info {
    (lazy: $func:expr) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Info as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Info, &($func)());
        }
    };
    ($($arg:tt)*) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Info as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Info, &format!($($arg)*));
        }
    };
}
pub(crate) use log_info;

macro_rules! log_warn {
    (lazy: $func:expr) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Warn as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Warn, &($func)());
        }
    };
    ($($arg:tt)*) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Warn as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Warn, &format!($($arg)*));
        }
    };
}
pub(crate) use log_warn;

macro_rules! log_error {
    (lazy: $func:expr) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Error as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Error, &($func)());
        }
    };
    ($($arg:tt)*) => {
        if $crate::utils::logger::MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
            >= $crate::utils::logger::LoggerLevel::Error as u8
        {
            $crate::utils::logger::emit_log($crate::bindings::LogLevel::Error, &format!($($arg)*));
        }
    };
}
pub(crate) use log_error;
