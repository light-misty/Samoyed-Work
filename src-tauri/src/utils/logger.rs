use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};

/// 双输出日志器，同时写入控制台(stderr)和日志文件
struct DualLogger {
    /// 日志文件句柄，使用 Mutex 保证线程安全；创建失败时为 None
    file: Option<Mutex<File>>,
    /// 最低日志级别
    level_filter: LevelFilter,
}

impl Log for DualLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level_filter
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let level = record.level();
        let module = record.module_path().unwrap_or("unknown");
        let message = format!(
            "{} [{}] {} - {}",
            timestamp,
            format_level(level),
            module,
            record.args()
        );

        // 写入标准错误输出（控制台）
        eprintln!("{}", message);

        // 写入日志文件
        if let Some(file) = &self.file {
            if let Ok(mut guard) = file.lock() {
                let _ = writeln!(guard, "{}", message);
            }
        }
    }

    fn flush(&self) {
        if let Some(file) = &self.file {
            if let Ok(mut guard) = file.lock() {
                let _ = guard.flush();
            }
        }
    }
}

/// 格式化日志级别为固定宽度字符串，便于对齐
fn format_level(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARN ",
        Level::Info => "INFO ",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

/// 初始化日志系统
///
/// - `log_dir`: 日志文件目录路径（相对于 CWD）
/// - 每次启动会覆盖上一次的日志文件（使用 Create + Truncate 模式）
/// - 开发模式(debug)日志级别为 DEBUG，发布模式(release)为 INFO
/// - 同时输出到控制台(stderr)和日志文件
/// - 如果日志文件创建失败，降级为仅控制台输出
pub fn init(log_dir: &Path) -> Result<(), SetLoggerError> {
    let level_filter = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    // 尝试创建日志目录和文件
    let file = match create_log_file(log_dir) {
        Ok(f) => Some(Mutex::new(f)),
        Err(e) => {
            eprintln!(
                "[日志] 无法创建日志文件，降级为仅控制台输出: {}",
                e
            );
            None
        }
    };

    let logger = Box::new(DualLogger {
        file,
        level_filter,
    });

    log::set_boxed_logger(logger)?;
    log::set_max_level(level_filter);

    match &log_dir.join("docagent.log").to_str() {
        Some(path) => log::info!("DocAgent 日志系统初始化完成，日志文件: {}", path),
        None => log::info!("DocAgent 日志系统初始化完成"),
    }

    Ok(())
}

/// 创建日志目录和文件
fn create_log_file(log_dir: &Path) -> Result<File, std::io::Error> {
    fs::create_dir_all(log_dir)?;
    let log_path = log_dir.join("docagent.log");
    File::create(&log_path)
}
