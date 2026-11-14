//! Laws of the OpenGL model: what the GL 1.1 specification states as properties, whatever
//! Windows' measured details turn out to be.
use kikizake::model::opengl::{Fog, FogMode};
use proptest::prelude::*;

fn fog(mode: FogMode, density: f32, start: f32, end: f32) -> Fog {
    Fog { mode, density, start, end, index: 0.0, color: [0.0; 4] }
}

proptest! {
    /// GL 1.1 3.9: the factor is held to 0..1, is 1 at the eye for the exponential modes, 1 at
    /// the start and 0 at the end for the linear one, never grows with the distance, and depends
    /// on the distance only (|z|)
    #[test]
    fn fog_factor_follows_the_modes(density in 0f32..8.0, start in 0f32..4.0, len in 0.01f32..4.0, z0 in 0f64..8.0, dz in 0f64..8.0) {
        let end = start + len;
        for mode in [FogMode::Exp, FogMode::Exp2, FogMode::Linear] {
            let f = fog(mode, density, start, end);
            let (a, b) = (f.factor(z0), f.factor(z0 + dz));
            prop_assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
            prop_assert!(b <= a, "{mode:?}: {a} at {z0}, {b} at {}", z0 + dz);
            prop_assert_eq!(f.factor(-z0), a);
            if mode != FogMode::Linear {
                prop_assert_eq!(f.factor(0.0), 1.0);
            }
        }
        let f = fog(FogMode::Linear, density, start, end);
        prop_assert_eq!(f.factor(f64::from(start)), 1.0);
        prop_assert!(f.factor(f64::from(end)).abs() < 1e-6);
    }

    /// GL 1.1 3.9: a mode's name round-trips and no other name is a mode
    #[test]
    fn fog_modes_round_trip(m in 0u32..0x3000) {
        match FogMode::try_from(m) {
            Ok(mode) => prop_assert_eq!(mode.name(), m),
            Err(()) => prop_assert!(![0x800, 0x801, 0x2601].contains(&m)),
        }
    }
}
