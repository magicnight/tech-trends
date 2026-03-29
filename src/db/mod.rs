mod schema;

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

pub use schema::run_migrations;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let db = Self { conn };
        run_migrations(&db.conn)?;
        Ok(db)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }
}
