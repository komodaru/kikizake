//! Kani proofs (`cargo kani`): clauses' arithmetic proved for every input of the stated domain,
//! independent of Windows' answers (spec/laws.txt lists them with the clauses they prove). The
//! models' own functions are called, so a proof fails when the model breaks the clause.
#![cfg(kani)]

use crate::model::{d3d7::{tri_z, Depth}, gdi::sample, gdiblend::alpha, gdixform::page};

fn ch(v: u32, i: u32) -> u32 {
    v >> (8 * i) & 255
}

/// GDI.STRETCH.SAMPLE / DDA: each destination pixel takes one source pixel inside the source,
/// in order (never going back), and equal extents take each pixel itself (GDI.STRETCH.SAME);
/// extents up to 64, a bound that keeps the division within the solver's reach
#[kani::proof]
fn stretch_samples_the_source_in_order() {
    let (i, sn, n, flat): (i32, i32, i32, bool) = (kani::any(), kani::any(), kani::any(), kani::any());
    kani::assume(sn >= 1 && sn <= 64 && n >= 1 && n <= 64 && i >= 0 && i < n - 1);
    let (a, b) = (sample(i, sn, n, flat), sample(i + 1, sn, n, flat));
    assert!(a >= 0 && b < sn && a <= b);
    if n == sn {
        assert!(a == i && b == i + 1);
    }
}

/// GDI.ALPHA.CONSTANT / ROUND: with a constant alpha each channel lies between the source's and
/// the destination's, 255 gives the source and 0 the destination
#[kani::proof]
fn constant_alpha_lies_between() {
    let (s, d, sca): (u32, u32, u32) = (kani::any(), kani::any(), kani::any());
    kani::assume(sca <= 255);
    let r = alpha(s, d, sca, false);
    for i in 0..4 {
        let (x, y, z) = (ch(s, i), ch(d, i), ch(r, i));
        assert!(z >= x.min(y) && z <= x.max(y));
        if sca == 255 {
            assert!(z == x);
        }
        if sca == 0 {
            assert!(z == y);
        }
    }
}

/// GDI.ALPHA.OPAQUE: a per-pixel source of alpha 255 blends as the constant alpha alone
#[kani::proof]
fn opaque_pixels_blend_as_constant() {
    let (s, d, sca): (u32, u32, u32) = (kani::any(), kani::any(), kani::any());
    kani::assume(sca <= 255 && s >> 24 == 255);
    assert!(alpha(s, d, sca, true) == alpha(s, d, sca, false));
}

/// D3D7.TRI / CLEAR.Z: a triangle's z value grows with z, 0 at 0 and the largest value at 1
#[kani::proof]
fn triangle_z_is_monotone() {
    let (z1, z2, wide): (f32, f32, bool) = (kani::any(), kani::any(), kani::any());
    kani::assume(z1 >= 0.0 && z1 <= z2 && z2 <= 1.0);
    let bits = if wide { Depth::Z32 } else { Depth::Z16 };
    assert!(tri_z(bits, z1) <= tri_z(bits, z2));
    if z2 == 1.0 {
        assert!(tri_z(bits, z2) == if wide { 0xffff_ff00 } else { 0xffff });
    }
    if z1 == 0.0 {
        assert!(tri_z(bits, z1) == 0);
    }
    // (the 16-bit z is the top of the 32-bit one)
    assert!(tri_z(Depth::Z16, z1) == tri_z(Depth::Z32, z1) >> 16);
}
