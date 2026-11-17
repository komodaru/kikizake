//! Each marked line is one regression; the mark after it (two slashes, a tilde) lists every counted
//! lint that must fire on that line
//! (the ratchet_tests of the lint crate). Nothing here runs.
#![allow(dead_code, unused)]

pub mod storage {
    pub struct Storage;
    impl Storage {
        pub fn get(&self, _logical: &str) -> Option<Vec<u8>> {
            None
        }
    }
}

pub enum E {
    A,
    B,
    C,
}

pub fn conversions(x: u64, y: i32, z: u32, f: fn() -> u32, p: *const u32) {
    let _ = x as u32; //~ as_conversions cast_possible_truncation
    let _ = y as u32; //~ as_conversions cast_sign_loss
    let _ = z as i32; //~ as_conversions cast_possible_wrap
    let _ = f as usize; //~ as_conversions fn_to_numeric_cast_any
    let _ = p as *const u8; //~ as_conversions ptr_as_ptr
}

pub fn wildcards(e: E) -> u32 {
    match e {
        E::A => 0,
        _ => 1, //~ wildcard_enum_match_arm
    }
}

pub fn single(e: E) -> u32 {
    match e {
        E::A => 0,
        E::B => 1,
        _ => 2, //~ match_wildcard_for_single_variants
    }
}

pub fn unwraps(o: Option<u32>, r: Result<u32, ()>) -> u32 {
    o.unwrap() //~ unwrap_used arithmetic_side_effects
        + r.expect("x") //~ expect_used
}

pub fn robust(v: &[u8], s: &str, a: u32, b: u32, m: &std::collections::HashMap<u32, u32>, r: Result<u32, ()>, fl: f64) -> u32 {
    let _ = v[0]; //~ indexing_slicing
    let _ = a + b; //~ arithmetic_side_effects
    let _ = &s[1..]; //~ string_slice
    let _ = v.get(0).unwrap(); //~ get_unwrap unwrap_used
    let _ = a as f32; //~ as_conversions cast_precision_loss
    let _ = 'a' as u8; //~ as_conversions char_lit_as_u8
    let _: u64 = a as _; //~ as_conversions as_underscore
    let _ = r; //~ let_underscore_must_use
    r.ok(); //~ unused_result_ok
    let _ = r.map_err(|_| 0u8); //~ map_err_ignore let_underscore_must_use
    let _ = fl == 1.5; //~ float_cmp
    let _: f32 = 16_777_217.0; //~ lossy_float_literal
    for x in m {} //~ iter_over_hash_type
    let _ = 2 ^ 3; //~ suspicious_xor_used_as_pow
    if a > 1 {
        panic!("x"); //~ panic
    }
    if a > 2 {
        todo!(); //~ todo
    }
    if a > 3 {
        unimplemented!(); //~ unimplemented
    }
    if a > 4 {
        unreachable!(); //~ unreachable
    }
    if a > 5 {
        std::process::exit(1); //~ exit
    }
    dbg!(a); //~ dbg_macro
    0
}

pub fn result_panics(a: u32) -> Result<u32, ()> { //~ panic_in_result_fn
    assert!(a > 0);
    Ok(a)
}

pub fn result_unwraps(r: Result<u32, ()>) -> Result<u32, ()> {
    let v = r.unwrap(); //~ unwrap_used unwrap_in_result
    Ok(v)
}

pub fn unsafes(p: *const u32, q: *mut u32) -> u32 {
    let a = unsafe { *p }; //~ undocumented_unsafe_blocks not_unsafe_ptr_arg_deref@correctness
    // SAFETY: fixture
    let b = unsafe { *p + *q }; //~ multiple_unsafe_ops_per_block arithmetic_side_effects not_unsafe_ptr_arg_deref
    let c = p.cast::<u8>() as *const u64; //~ as_conversions ptr_as_ptr cast_ptr_alignment
    let d = p as *mut u32; //~ as_conversions ptr_cast_constness
    let x = 5u32;
    let e = &x as *const u32; //~ as_conversions borrow_as_ptr
    0
}

/// no Safety section
pub unsafe fn no_doc(p: *const u32) -> u32 { //~ missing_safety_doc
    *p //~ unsafe_op_in_unsafe_fn
}

pub fn transmutes(p: *const u32, v: &[u16]) {
    // SAFETY: fixture
    let _ = unsafe { std::mem::transmute::<*const u32, *const u8>(p) }; //~ masu-transmute transmute_ptr_to_ptr not_unsafe_ptr_arg_deref
    // SAFETY: fixture
    let _ = unsafe { std::mem::transmute::<Vec<i32>, Vec<u32>>(vec![]) }; //~ masu-transmute transmute_undefined_repr
    let s: &[u32] = &[1];
    let _ = s as *const [u32] as *const [u8]; //~ as_conversions cast_slice_different_sizes
}

pub fn more(a: i64, now: std::time::Instant, earlier: std::time::Instant, d: std::time::Duration) {
    let _ = (300i32 as u8); //~ as_conversions cast_possible_truncation
    let _ = now - d; //~ unchecked_time_subtraction arithmetic_side_effects
    std::mem::forget(vec![1u8]); //~ mem_forget
    let big = [0u8; 1 << 20]; //~ large_stack_arrays
    let w: Vec<u32> = (0..).take_while(|x| *x < 5).collect(); //~ maybe_infinite_iter
    let mut v = 0u8;
    let _ = { v = 1; v } + v; //~ mixed_read_write_in_expression arithmetic_side_effects
    let mut w = vec![1u8];
    debug_assert!(w.pop().is_some()); //~ debug_assert_with_mut_call
    if (a as u32) < (u32::MAX as u32) {} //~ as_conversions cast_possible_truncation cast_sign_loss
}

pub enum Small {
    A = 1,
    B = 300,
}

pub fn enums(s: Small) {
    let _ = Small::B as u8; //~ as_conversions cast_enum_truncation
}

pub fn fn_trunc(f: fn()) {
    let _ = f as u16; //~ as_conversions fn_to_numeric_cast_any fn_to_numeric_cast_with_truncation
}

pub fn upcast(x: u8) -> bool {
    (x as u32) > 300 //~ as_conversions invalid_upcast_comparisons
}

pub fn checked(x: u64) -> bool {
    x <= u64::from(u32::MAX) //~ checked_conversions
}

pub struct Wrap(u32);
impl From<Option<u32>> for Wrap { //~ fallible_impl_from
    fn from(o: Option<u32>) -> Self {
        Wrap(o.unwrap()) //~ unwrap_used
    }
}

pub struct D;
impl Drop for D { //~ empty_drop
    fn drop(&mut self) {}
}

pub trait T {
    fn same(&self);
}
pub struct S { pub a: u32 }
impl S {
    pub fn same(&self) {} //~ same_name_method
}
impl T for S {
    fn same(&self) {}
}

pub fn rest(s: S) -> u32 {
    let S { a, .. } = s; //~ rest_pat_in_fully_bound_structs
    a
}

pub fn or_wild(e: E) -> u32 {
    match e {
        E::A => 0,
        E::B | _ => 1, //~ wildcard_in_or_patterns
    }
}

pub fn needless_safety() {
    // SAFETY: nothing unsafe here
    let x = 1; //~ unnecessary_safety_comment
}

pub fn try_err(a: u32) -> Result<u32, u32> {
    if a > 0 {
        Err(a)?; //~ try_err
    }
    Ok(a)
}

pub fn mut_mut(a: &mut &mut u32) {} //~ mut_mut

pub fn correctness(x: u32) -> bool {
    x == x //~ eq_op@correctness
}

pub fn suspicious(v: &[u32]) -> usize {
    v.iter().map(|x| x + 1).count() //~ suspicious_map@suspicious arithmetic_side_effects
}

pub fn perf() -> Vec<u32> {
    let mut v = Vec::new(); //~ vec_init_then_push@perf
    v.push(1);
    v
}

pub fn uninit() -> u32 {
    // SAFETY: fixture
    unsafe { std::mem::zeroed() } //~ masu-uninit
}

pub fn raw(p: *const u8) -> u8 {
    // SAFETY: fixture
    let s = unsafe { std::slice::from_raw_parts(p, 1) }; //~ masu-from_raw_parts not_unsafe_ptr_arg_deref
    s.first().copied().unwrap_or(0)
}

pub fn set_len(v: &mut Vec<u8>) {
    // SAFETY: fixture
    unsafe { v.set_len(0) } //~ masu-set_len
}

static mut COUNT: u32 = 0; //~ masu-static_mut

pub struct Raw(*mut u8);
// SAFETY: fixture
unsafe impl Send for Raw {} //~ masu-unsafe_send_sync

pub const TREE: &str = "vnvm-tree"; //~ masu-wire_tree

// (an #[allow] counts as its own lint, and does not hide what it allows: clippy runs with
// --force-warn)
#[allow(clippy::unwrap_used)] //~ allow-attr
pub fn silenced(o: Option<u32>) -> u32 {
    o.unwrap() //~ unwrap_used
}

#[allow( //~ allow-attr
    dead_code,
    clippy::unwrap_used,
)]
pub fn silenced_over_lines(o: Option<u32>) -> u32 {
    o.unwrap() //~ unwrap_used
}

#[cfg_attr(all(), expect(clippy::unwrap_used))] //~ allow-attr
pub fn silenced_by_cfg_attr(o: Option<u32>) -> u32 {
    o.unwrap() //~ unwrap_used
}

macro_rules! silenced_in_macro {
    () => {
        #[allow(clippy::unwrap_used)] //~ allow-attr
        pub fn silenced_in_macro(o: Option<u32>) -> u32 {
            o.unwrap() // (clippy does not look inside a macro's expansion; the attribute still counts)
        }
    };
}
silenced_in_macro!();

#[cfg_attr(not(test), allow(dead_code))]
pub fn allows_only_an_untracked_lint() {}
