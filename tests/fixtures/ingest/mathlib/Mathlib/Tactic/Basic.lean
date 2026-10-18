/-
Hand-written test fixture: an infrastructure module (tactics), in the style of Mathlib.
-/
module

public meta import Lean.Elab.Command

/-! # Basic tactics (fixture) -/

namespace Mathlib.Tactic

/-- Elaborator helper: infrastructure, not mathematics. -/
def elabHelper (n : Nat) : Nat := n

end Mathlib.Tactic
