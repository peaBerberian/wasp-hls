use std::sync::atomic::AtomicU8;

use crate::bindings::{LogLevel, jsLog};

static MAX_LOG_LEVEL: AtomicU8 = AtomicU8::new(4);

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum LoggerLevel {
    None = 0,
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

/// TODO Explore other logging functions implementations.

pub struct Logger {}

impl Logger {
    pub fn set_logger_level(new_level: LoggerLevel) {
        MAX_LOG_LEVEL.store(new_level as u8, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn info(text: &str) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Info as u8 {
            jsLog(LogLevel::Info, text);
        }
    }

    pub fn error(text: &str) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Error as u8 {
            jsLog(LogLevel::Error, text);
        }
    }

    pub fn warn(text: &str) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Warn as u8 {
            jsLog(LogLevel::Warn, text);
        }
    }

    pub fn debug(text: &str) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Debug as u8 {
            jsLog(LogLevel::Debug, text);
        }
    }

    pub fn lazy_info(func: &dyn Fn() -> String) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Info as u8 {
            jsLog(LogLevel::Info, &func());
        }
    }

    pub fn lazy_error(func: &dyn Fn() -> String) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Error as u8 {
            jsLog(LogLevel::Error, &func());
        }
    }

    pub fn lazy_warn(func: &dyn Fn() -> String) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Warn as u8 {
            jsLog(LogLevel::Warn, &func());
        }
    }

    pub fn lazy_debug(func: &dyn Fn() -> String) {
        if MAX_LOG_LEVEL.load(std::sync::atomic::Ordering::Relaxed) >= LoggerLevel::Debug as u8 {
            jsLog(LogLevel::Debug, &func());
        }
    }
}
