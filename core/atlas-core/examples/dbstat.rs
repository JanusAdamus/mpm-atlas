//! Bytes per table and index of a registry (SQLite `dbstat`, bundled build), as one JSON object.
//! `cargo run --release -p atlas-core --example dbstat -- data/pilot/atlas-pilot.db`
use rusqlite::{Connection, OpenFlags};

fn main() -> rusqlite::Result<()> {
    let db = std::env::args().nth(1).expect("usage: dbstat <db>");
    let t = std::time::Instant::now();
    let c = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let page: i64 = c.query_row("PRAGMA page_size", [], |r| r.get(0))?;
    let pages: i64 = c.query_row("PRAGMA page_count", [], |r| r.get(0))?;
    let free: i64 = c.query_row("PRAGMA freelist_count", [], |r| r.get(0))?;
    let mut st = c.prepare("SELECT name, SUM(pgsize), COUNT(*) FROM dbstat GROUP BY name ORDER BY 2 DESC")?;
    let objs: Vec<serde_json::Value> = st
        .query_map([], |r| Ok(serde_json::json!({"name": r.get::<_, String>(0)?, "bytes": r.get::<_, i64>(1)?, "pages": r.get::<_, i64>(2)?})))?
        .collect::<rusqlite::Result<_>>()?;
    let out = serde_json::json!({"db": db, "file_bytes": page * pages, "free_bytes": free * page, "page_size": page,
                                 "objects": objs, "seconds": t.elapsed().as_secs_f64()});
    println!("{out}");
    Ok(())
}
