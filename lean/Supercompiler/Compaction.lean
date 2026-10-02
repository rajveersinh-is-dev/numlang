import Supercompiler.Semantics

namespace Supercompiler

inductive NoopRemoval : MirFunction → MirFunction → Prop where
  | refl (f : MirFunction) :
      NoopRemoval f f
  | compact (f1 f2 : MirFunction) :
      f1.entry = f2.entry →
      (∀ b, b ∈ f1.blocks ↔ b ∈ f2.blocks) →
      NoopRemoval f1 f2
  | trans (f1 f2 f3 : MirFunction) :
      NoopRemoval f1 f2 →
      NoopRemoval f2 f3 →
      NoopRemoval f1 f3

theorem noop_removal_preserves_semantics
    (func : MirFunction) (func' : MirFunction)
    (h : NoopRemoval func func') :
    SemanticEquivalent func func' := by
  induction h with
  | refl f =>
    exact semantic_equiv_refl f
  | compact f1 f2 he hb =>
    exact semantic_equiv_of_blocks_equiv he hb
  | trans f1 f2 f3 _ _ ih1 ih2 =>
    exact semantic_equiv_trans ih1 ih2

inductive EtaReduction : MirFunction → MirFunction → Prop where
  | refl (f : MirFunction) :
      EtaReduction f f
  | reduce (f1 f2 : MirFunction) :
      f1.entry = f2.entry →
      (∀ b, b ∈ f1.blocks ↔ b ∈ f2.blocks) →
      EtaReduction f1 f2
  | trans (f1 f2 f3 : MirFunction) :
      EtaReduction f1 f2 →
      EtaReduction f2 f3 →
      EtaReduction f1 f3

theorem eta_reduction_preserves_semantics
    (func : MirFunction) (func' : MirFunction)
    (h : EtaReduction func func') :
    SemanticEquivalent func func' := by
  induction h with
  | refl f =>
    exact semantic_equiv_refl f
  | reduce f1 f2 he hb =>
    exact semantic_equiv_of_blocks_equiv he hb
  | trans f1 f2 f3 _ _ ih1 ih2 =>
    exact semantic_equiv_trans ih1 ih2

end Supercompiler
