// Throughput probe: sessions x intra threads, batch size, sorted lengths.
use fastembed::{EmbeddingModel, TextInitOptions, TextEmbedding};
fn main() {
    let args: Vec<usize> = std::env::args().skip(1).map(|a| a.parse().unwrap()).collect();
    let (sessions, intra, batch, n) = (args[0], args[1], args[2], args[3]);
    let words = ["teorema","grupo","espacio","vectorial","compacto","topología","integral","serie","convergencia","Hilbert","prime","number","field","ring","module","homology","manifold","smooth","measure","probability"];
    let texts: Vec<String> = (0..n).map(|i| { let k = 6 + (i * 7) % 20; (0..k).map(|j| words[(i * 31 + j * 17) % words.len()]).collect::<Vec<_>>().join(" ") }).map(|t| format!("passage: {t}")).collect();
    let t = std::time::Instant::now();
    let models: Vec<_> = (0..sessions).map(|_| std::sync::Mutex::new(TextEmbedding::try_new(TextInitOptions::new(EmbeddingModel::MultilingualE5Small).with_max_length(128).with_intra_threads(intra).with_cache_dir("C:/Users/AdrianGonzalez/.cache/atlas-models".into())).unwrap())).collect();
    println!("load {:?}", t.elapsed());
    let t = std::time::Instant::now();
    let chunks: Vec<&[String]> = texts.chunks(batch).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| for m in &models { s.spawn(|| loop { let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed); if i >= chunks.len() { break } m.lock().unwrap().embed(chunks[i], Some(batch)).unwrap(); }); });
    let secs = t.elapsed().as_secs_f64();
    println!("sessions {sessions} intra {intra} batch {batch}: {n} texts in {secs:.2}s = {:.0}/s", n as f64 / secs);
}
