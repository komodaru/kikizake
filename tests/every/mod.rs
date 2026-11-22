//! The whole 32-bit domain on the machine's threads, for the exhaustive laws over every float.

/// runs `f` over every u32 on the machine's threads; the first failing value, if any
pub fn all_u32(f: impl Fn(u32) -> bool + Sync) -> Option<u32> {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = (1u64 << 32) / n as u64;
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..n as u64)
            .map(|k| {
                let f = &f;
                s.spawn(move || {
                    let lo = k * chunk;
                    let hi = if k + 1 == n as u64 { 1u64 << 32 } else { lo + chunk };
                    (lo..hi).map(|v| v as u32).find(|&v| !f(v))
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().unwrap()).min()
    })
}
