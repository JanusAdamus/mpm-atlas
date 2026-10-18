/-
Hand-written test fixture in the style of Mathlib (not copied from it).
-/
module

public import Mathlib.Data.Nat.Prime.Defs

/-!
# The Pythagorean theorem (fixture)
-/

namespace EuclideanGeometry

/-- **Pythagorean theorem**: in a right triangle the square of the hypotenuse equals the sum of the
squares of the legs. -/
theorem dist_sq_eq_dist_sq_add_dist_sq_iff_angle_eq_pi_div_two (p1 p2 p3 : P) :
    dist p1 p3 * dist p1 p3 = dist p1 p2 * dist p1 p2 + dist p3 p2 * dist p3 p2 ↔
      ∠ p1 p2 p3 = π / 2 := by
  sorry

/-- A right angle, used to state the Pythagorean theorem. -/
def RightAngle (p1 p2 p3 : P) : Prop := ∠ p1 p2 p3 = π / 2

theorem rightAngle_of_prime (h : Nat.Prime 2) (p1 p2 p3 : P) : RightAngle p1 p2 p3 → True := fun _ => trivial

end EuclideanGeometry
