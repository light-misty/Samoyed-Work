pub mod init;
pub mod session_repo;
pub mod message_repo;
pub mod snapshot_repo;
pub mod token_repo;

use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;
use crate::errors::CommandError;

/// 数据库封装，内部持有 Mutex 保护的 SQLite 连接
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// 创建或打开数据库文件，启用 WAL 模式和外键约束，并执行初始化
    pub fn new(db_path: &Path) -> Result<Self, CommandError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self {
            conn: Mutex::new(conn),
        };
        db.initialize()?;
        Ok(db)
    }

    /// 执行数据库初始化（建表、索引、版本记录）
    fn initialize(&self) -> Result<(), CommandError> {
        let conn = self
            .conn
            .lock()
            .map_err(|e| CommandError::db(crate::errors::DB_CONNECTION_FAILED, e.to_string()))?;
        init::initialize_database(&conn)?;
        Ok(())
    }

    /// 获取 MutexGuard 保护下的数据库连接
    pub fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CommandError> {
        self.conn
            .lock()
            .map_err(|e| CommandError::db(crate::errors::DB_CONNECTION_FAILED, e.to_string()))
    }
}
