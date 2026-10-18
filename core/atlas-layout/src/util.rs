//! Small deterministic helpers: a seeded RNG and order-stable parallel sums.
//!
//! Everything in this crate must give bit-identical output for the same input and
//! seed, whatever the thread count.  rayon's own `sum()` on floats depends on how
//! work was split, so float reductions go through `det_sum` (fixed chunks, summed
//! in order).

use rayon::prelude::*;

/// Fixed chunk size for deterministic parallel work.
pub(crate) const CHUNK: usize = 4096;

/// splitmix64: tiny, seedable, good enough for shuffles and tie breaking.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

pub(crate) fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    /// A stream keyed by several integers (seed, round, level, community...).
    pub(crate) fn keyed(parts: &[u64]) -> Rng {
        Rng(parts.iter().fold(0x5EED_u64, |h, &p| mix(h ^ p)))
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        mix(self.0)
    }

    /// Uniform in [0, 1).
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(crate) fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    pub(crate) fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            v.swap(i, self.below(i + 1));
        }
    }
}

/// Sum of `f(i)` for `i in 0..n`, identical for any thread count.
pub(crate) fn det_sum(n: usize, f: impl Fn(usize) -> f64 + Sync) -> f64 {
    let parts: Vec<f64> = (0..n.div_ceil(CHUNK))
        .into_par_iter()
        .map(|c| (c * CHUNK..((c + 1) * CHUNK).min(n)).map(&f).sum())
        .collect();
    parts.iter().sum()
}

/// Two sums at once (same guarantees as `det_sum`).
pub(crate) fn det_sum2(n: usize, f: impl Fn(usize) -> (f64, f64) + Sync) -> (f64, f64) {
    let parts: Vec<(f64, f64)> = (0..n.div_ceil(CHUNK))
        .into_par_iter()
        .map(|c| {
            (c * CHUNK..((c + 1) * CHUNK).min(n))
                .map(&f)
                .fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1))
        })
        .collect();
    parts.iter().fold((0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1))
}

/// Nodes grouped by community: `order[start[c]..start[c+1]]` are the members of `c`
/// in increasing node order.
pub(crate) fn group_by(membership: &[u32], count: usize) -> (Vec<u32>, Vec<usize>) {
    let mut start = vec![0usize; count + 1];
    for &c in membership {
        start[c as usize + 1] += 1;
    }
    for c in 0..count {
        start[c + 1] += start[c];
    }
    let mut fill = start.clone();
    let mut order = vec![0u32; membership.len()];
    for (v, &c) in membership.iter().enumerate() {
        order[fill[c as usize]] = v as u32;
        fill[c as usize] += 1;
    }
    (order, start)
}

/// Renumber community ids to 0..k in order of first appearance; returns k.
pub(crate) fn renumber(membership: &mut [u32]) -> usize {
    let max = membership
        .iter()
        .copied()
        .max()
        .map_or(0, |m| m as usize + 1);
    let mut map = vec![u32::MAX; max];
    let mut k = 0u32;
    for c in membership.iter_mut() {
        let slot = &mut map[*c as usize];
        if *slot == u32::MAX {
            *slot = k;
            k += 1;
        }
        *c = *slot;
    }
    k as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn det_sum_is_order_stable() {
        let f = |i: usize| 1.0 / (i as f64 + 1.0);
        let a = det_sum(100_000, f);
        let b = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap()
            .install(|| det_sum(100_000, f));
        assert_eq!(a.to_bits(), b.to_bits());
    }

    #[test]
    fn renumber_first_appearance() {
        let mut m = vec![7, 7, 3, 9, 3];
        assert_eq!(renumber(&mut m), 3);
        assert_eq!(m, vec![0, 0, 1, 2, 1]);
        let (order, start) = group_by(&m, 3);
        assert_eq!((order, start), (vec![0, 1, 2, 4, 3], vec![0, 2, 4, 5]));
    }
}
