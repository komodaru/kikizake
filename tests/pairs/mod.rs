//! Pairs of two sequences that must be as long as each other: a comparison zipped with `zip`
//! stops at the shorter side, so an output that lost its lines compares nothing and passes (the
//! review of 84b05c8, 4.3: placements_are_the_scaled_advance passed with every `place` line
//! removed). kikizake-lint `zip-unchecked` keeps tests/ on this.

/// the pairs of `a` and `b`, which must have the same length (the test fails here if not)
#[track_caller]
pub fn zip_eq<A: IntoIterator, B: IntoIterator>(a: A, b: B) -> std::iter::Zip<std::vec::IntoIter<A::Item>, std::vec::IntoIter<B::Item>> {
    let (a, b): (Vec<A::Item>, Vec<B::Item>) = (a.into_iter().collect(), b.into_iter().collect());
    assert_eq!(a.len(), b.len(), "zipped sequences of different lengths");
    a.into_iter().zip(b)
}
