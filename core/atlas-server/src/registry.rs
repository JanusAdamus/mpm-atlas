//! Read-only connections to the registry (`data/atlas.db`) for detail views and the
//! lexical fallback.  One connection per blocking-pool thread, opened on first use
//! (SPEC §12: one connection per thread; readers open read-only).

use atlas_core::Result;
use rusqlite::{Connection, OpenFlags};
use std::cell::RefCell;
use std::path::{Path, PathBuf};

const MMAP_BYTES: i64 = 1 << 36;
const CACHE_KIB: i64 = 64 * 1024;

thread_local! {
    static CONN: RefCell<Option<(PathBuf, Connection)>> = const { RefCell::new(None) };
}

fn open(db: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI;
    let conn = Connection::open_with_flags(db, flags)?;
    conn.pragma_update(None, "query_only", true)?;
    conn.pragma_update(None, "mmap_size", MMAP_BYTES)?;
    conn.pragma_update(None, "cache_size", -CACHE_KIB)?;
    Ok(conn)
}

/// Run `f` with this thread's connection to `db`.
pub fn with<T>(db: &Path, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    CONN.with_borrow_mut(|slot| {
        if slot.as_ref().is_none_or(|(p, _)| p != db) {
            *slot = Some((db.to_path_buf(), open(db)?));
        }
        f(&slot.as_ref().expect("connection just opened").1)
    })
}

/// Startup check: the file opens read-only and has the v1 tables.
pub fn probe(db: &Path) -> Result<bool> {
    with(db, |c| {
        let fts: i64 = c.query_row("SELECT EXISTS(SELECT 1 FROM fts LIMIT 1)", [], |r| r.get(0))?;
        c.query_row("SELECT 1 FROM entities LIMIT 1", [], |_| Ok(())).or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(()),
            e => Err(e),
        })?;
        Ok(fts == 1)
    })
}
