//! D3DX's DXT encoder (spec/d3dx9.txt D3DX9.LOAD.NEGFAULT, LOAD.DXTENC): OptimizeRGB's step of a
//! point for every f32 dot, on both builds (kind exhaustive).
use kikizake::model::dxtenc::{step_index, Below, Real, X87};

mod every;
use every::all_u32;

/// D3DX9.LOAD.NEGFAULT: for every f32 dot and both step counts, x64 (f32) and x86 (the x87
/// rounding toward zero, as in OptimizeRGB's loop): a dot not below the steps' count (or NaN)
/// takes the last step; one at or below -1.5 has the index -1, x64's access violation and x86's
/// read below the table; any other the step trunc(dot + 0.5), the sum rounded as the build
/// rounds, one of the table's
#[test]
fn the_step_of_every_dot() {
    for n in [3usize, 4] {
        let fs = if n == 3 { 2.0f32 } else { 3.0 };
        assert_eq!(all_u32(|w| {
            let dot = f32::from_bits(w);
            let low = f64::from(dot) <= -1.5;
            let x64 = step_index(dot, fs, n);
            X87::chop(true);
            let x86 = step_index(X87::of(dot), X87::of(fs), n);
            X87::chop(false);
            if !(fs > dot) {
                return x64 == Ok(n - 1) && x86 == Ok(n - 1);
            }
            if low {
                return x64 == Err(Below::Fault) && x86 == Err(Below::Unknown);
            }
            let near = (dot + 0.5).trunc();
            // (x86's sum to 24 bits toward zero: never past the exact sum, so trunc of the exact)
            let chop = (f64::from(dot) + 0.5).trunc();
            let ok = |r: Result<usize, Below>, want: f64| r.is_ok_and(|i| i < n && f64::from(u8::try_from(i).unwrap_or(255)) == want);
            ok(x64, f64::from(near)) && ok(x86, chop)
        }), None, "{n}");
    }
}
