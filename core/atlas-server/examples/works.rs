//! The works-layer commands of `atlas` (`works-compile`, `serve --works`) as a separate binary, so
//! they run while the user's server holds `atlas.exe` open.
//! `works compile <db> <core snapshot dir> <out dir> [threads]`
//! `works serve <core snapshot dir> <db> <works dir> <addr> [index dir]`
fn main() -> atlas_core::Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.first().map(String::as_str) {
        Some("compile") => {
            let threads = a.get(4).and_then(|t| t.parse().ok()).unwrap_or(0);
            let v = atlas_server::works::compile(a[1].as_ref(), a[2].as_ref(), a[3].as_ref(), threads)?;
            println!("{v}");
        }
        Some("serve") => {
            let mut cfg = atlas_server::ServerConfig::new(&a[1]);
            cfg.db = Some(a[2].clone().into());
            cfg.works_dir = Some(a[3].clone().into());
            cfg.index_dir = a.get(5).map(Into::into);
            let addr: std::net::SocketAddr = a[4].parse().map_err(|e| atlas_core::Error::BadRequest(format!("addr: {e}")))?;
            let state = atlas_server::AppState::load(&cfg)?;
            cfg.runtime()?.block_on(atlas_server::serve(addr, state))?;
        }
        _ => eprintln!("usage: works compile <db> <core> <out> [threads] | works serve <core> <db> <works> <addr> [index]"),
    }
    Ok(())
}
