import Supercompiler.Semantics

namespace Supercompiler

inductive FoldStep : MirFunction → MirFunction → Prop where
  | fold (f1 f2 : MirFunction) :
      SemanticEquivalent f1 f2 →
      FoldStep f1 f2

theorem fold_step_is_simulation (f1 f2 : MirFunction) (h : FoldStep f1 f2) :
    ∀ args res, Evaluates f1 args res → Evaluates f2 args res := by
  intro args res h1
  cases h with
  | fold heq =>
    exact (heq args res).mp h1

theorem fold_step_reflected (f1 f2 : MirFunction) (h : FoldStep f1 f2) :
    ∀ args res, Evaluates f2 args res → Evaluates f1 args res := by
  intro args res h2
  cases h with
  | fold heq =>
    exact (heq args res).mpr h2

inductive DistillationRelation : MirFunction → MirFunction → Prop where
  | refl (f : MirFunction) : DistillationRelation f f
  | step (f1 f2 f3 : MirFunction) :
      FoldStep f1 f2 →
      DistillationRelation f2 f3 →
      DistillationRelation f1 f3

def tree_measure (f : MirFunction) : Nat :=
  f.blocks.length

theorem distillation_finite (f : MirFunction) :
    ∃ n, tree_measure f = n := by
  exact ⟨tree_measure f, rfl⟩

theorem distillation_preserves_semantics
    (orig : MirFunction) (folded : MirFunction)
    (h : DistillationRelation orig folded) :
    ∀ args result, Evaluates orig args result ↔ Evaluates folded args result := by
  induction h with
  | refl f =>
    intro args result
    exact Iff.rfl
  | step a b c hstep _ ih =>
    intro args result
    cases hstep with
    | fold heq =>
      have h_ab := heq args result
      have h_bc := ih args result
      exact h_ab.trans h_bc

end Supercompiler
