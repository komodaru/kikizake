import Kikizake.Mirror
/-! Address arithmetic for GL.EVAL. The caller supplies strides in scalar components.
    The Rust map definition walks u first, then v, copying width consecutive components
    starting at i * uStride + j * vStride. The strides may overlap or include padding. -/
namespace Kikizake.OpenGL.Evaluator

def index (i j uStride vStride component : Nat) : Nat :=
  i * uStride + j * vStride + component

/-- For nonempty axes and a nonempty point, all control components fit exactly when
    the last component at the last grid point fits. Includes overlapping strides. -/
theorem gridFits (u v w uStride vStride capacity : Nat) :
    (∀ i ≤ u, ∀ j ≤ v, ∀ k ≤ w, index i j uStride vStride k < capacity) ↔
      index u v uStride vStride w < capacity := by
  constructor
  · intro h
    exact h u (Nat.le_refl _) v (Nat.le_refl _) w (Nat.le_refl _)
  · intro last i hi j hj k hk
    exact Nat.lt_of_le_of_lt
      (Nat.add_le_add (Nat.add_le_add (Nat.mul_le_mul_right _ hi)
        (Nat.mul_le_mul_right _ hj)) hk) last

def mirrors : List Mirror := [
  ⟨"gl.control_index", fun args => do
    let [i, j, us, vs, k] := args | none
    return [Int.ofNat (index (← nat i) (← nat j) (← nat us) (← nat vs) (← nat k))]⟩]

end Kikizake.OpenGL.Evaluator
