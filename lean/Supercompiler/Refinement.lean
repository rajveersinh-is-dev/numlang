import Supercompiler.Semantics

namespace Supercompiler

abbrev SymTermId := Nat

structure Interval where
  lo : Option Int
  hi : Option Int
  deriving Repr, DecidableEq

inductive BranchDirection where
  | ThenBranch
  | ElseBranch
  deriving Repr, DecidableEq

structure SymbolicState where
  refinements : List (SymTermId × Interval)
  deriving Repr, DecidableEq

def lookupRefinement (l : List (SymTermId × Interval)) (id : SymTermId) : Interval :=
  match l with
  | [] => { lo := none, hi := none }
  | (k, iv) :: rest =>
    if k = id then iv else lookupRefinement rest id

def SymbolicState.get_refinement (state : SymbolicState) (id : SymTermId) : Interval :=
  lookupRefinement state.refinements id

def interval_impossible (iv : Interval) (dir : BranchDirection) : Prop :=
  match dir with
  | BranchDirection.ThenBranch => iv.hi = some 0 ∧ iv.lo = some 0
  | BranchDirection.ElseBranch => ∃ n : Int, n ≠ 0 ∧ iv.lo = some n ∧ iv.hi = some n

def satisfies_refinement (env : MirEnv) (state : SymbolicState) : Prop :=
  ∀ id iv, state.get_refinement id = iv →
    ∀ n : Int, lookup env (toString id) = some (Val.intVal n) →
      (∀ lo, iv.lo = some lo → lo ≤ n) ∧
      (∀ hi, iv.hi = some hi → n ≤ hi)

def branch_taken (env : MirEnv) (cond : SymTermId) (dir : BranchDirection) : Prop :=
  match dir with
  | BranchDirection.ThenBranch =>
    ∃ n : Int, lookup env (toString cond) = some (Val.intVal n) ∧ n ≠ 0
  | BranchDirection.ElseBranch =>
    lookup env (toString cond) = some (Val.intVal 0)

theorem refinement_pruning_sound
    (cond : SymTermId) (iv : Interval) (branch_dir : BranchDirection)
    (state : SymbolicState)
    (h_refine : state.get_refinement cond = iv)
    (h_impossible : interval_impossible iv branch_dir) :
    ∀ concrete_env, satisfies_refinement concrete_env state →
      ¬ branch_taken concrete_env cond branch_dir := by
  intro concrete_env h_sat h_taken
  cases branch_dir with
  | ThenBranch =>
    dsimp [interval_impossible] at h_impossible
    rcases h_impossible with ⟨h_hi, h_lo⟩
    dsimp [branch_taken] at h_taken
    rcases h_taken with ⟨n, h_lookup, h_nz⟩
    have h_bounds := h_sat cond iv h_refine n h_lookup
    rcases h_bounds with ⟨h_ge, h_le⟩
    have h_le0 := h_le 0 h_hi
    have h_ge0 := h_ge 0 h_lo
    have h_eq : n = 0 := by
      exact Int.le_antisymm h_le0 h_ge0
    exact h_nz h_eq
  | ElseBranch =>
    dsimp [interval_impossible] at h_impossible
    rcases h_impossible with ⟨m, h_mnz, h_lo, h_hi⟩
    dsimp [branch_taken] at h_taken
    have h_bounds := h_sat cond iv h_refine 0 h_taken
    rcases h_bounds with ⟨h_ge, h_le⟩
    have h_m_le_0 := h_ge m h_lo
    have h_0_le_m := h_le m h_hi
    have h_m_eq_0 : m = 0 := by
      exact Int.le_antisymm h_m_le_0 h_0_le_m
    exact h_mnz h_m_eq_0

end Supercompiler
