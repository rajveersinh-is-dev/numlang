import Supercompiler.Semantics

namespace Supercompiler

inductive FoldStep : MirFunction → MirFunction → Prop where
  | fold (f1 f2 : MirFunction) :
      f1.entry = f2.entry →
      (∀ args res, Evaluates f1 args res → Evaluates f2 args res) →
      (∀ args res, Evaluates f2 args res → Evaluates f1 args res) →
      FoldStep f1 f2

theorem fold_step_preserves_semantics (f1 f2 : MirFunction) (h : FoldStep f1 f2) :
    SemanticEquivalent f1 f2 := by
  cases h with
  | fold _ hf hr =>
    intro args res
    exact ⟨hf args res, hr args res⟩

theorem fold_step_is_simulation (f1 f2 : MirFunction) (h : FoldStep f1 f2) :
    ∀ args res, Evaluates f1 args res → Evaluates f2 args res := by
  intro args res h1
  have heq := fold_step_preserves_semantics f1 f2 h
  exact (heq args res).mp h1

theorem fold_step_reflected (f1 f2 : MirFunction) (h : FoldStep f1 f2) :
    ∀ args res, Evaluates f2 args res → Evaluates f1 args res := by
  intro args res h2
  have heq := fold_step_preserves_semantics f1 f2 h
  exact (heq args res).mpr h2

inductive DistillationRelation : MirFunction → MirFunction → Prop where
  | refl (f : MirFunction) : DistillationRelation f f
  | step (f1 f2 f3 : MirFunction) :
      FoldStep f1 f2 →
      DistillationRelation f2 f3 →
      DistillationRelation f1 f3

theorem distillation_preserves_semantics
    (orig : MirFunction) (folded : MirFunction)
    (h : DistillationRelation orig folded) :
    ∀ args result, Evaluates orig args result ↔ Evaluates folded args result := by
  induction h with
  | refl f =>
    intro args result
    exact Iff.rfl
  | step a b _ hstep _ ih =>
    intro args result
    have heq := fold_step_preserves_semantics a b hstep
    have h_ab := heq args result
    have h_bc := ih args result
    exact h_ab.trans h_bc

end Supercompiler
