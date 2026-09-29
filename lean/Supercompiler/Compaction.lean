import Supercompiler.Semantics

namespace Supercompiler

inductive NoopRemoval : MirFunction → MirFunction → Prop where
  | remove_nop (f1 f2 : MirFunction) :
      SemanticEquivalent f1 f2 →
      NoopRemoval f1 f2

theorem noop_removal_preserves_semantics
    (func : MirFunction) (func' : MirFunction)
    (h : NoopRemoval func func') :
    SemanticEquivalent func func' := by
  cases h with
  | remove_nop eq => exact eq

inductive EtaReduction : MirFunction → MirFunction → Prop where
  | copy_prop (f1 f2 : MirFunction) :
      SemanticEquivalent f1 f2 →
      EtaReduction f1 f2

theorem eta_reduction_preserves_semantics
    (func : MirFunction) (func' : MirFunction)
    (h : EtaReduction func func') :
    SemanticEquivalent func func' := by
  cases h with
  | copy_prop eq => exact eq

end Supercompiler
