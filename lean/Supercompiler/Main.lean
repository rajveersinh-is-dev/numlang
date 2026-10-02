import Supercompiler.Semantics
import Supercompiler.Preservation
import Supercompiler.Distillation
import Supercompiler.MRSC
import Supercompiler.Refinement
import Supercompiler.Compaction

namespace Supercompiler

inductive FunctionTransformed : MirFunction → MirFunction → Prop where
  | driving (f1 f2 : MirFunction) : MultiDriveStep f1 f2 → FunctionTransformed f1 f2
  | distillation (f1 f2 : MirFunction) : DistillationRelation f1 f2 → FunctionTransformed f1 f2
  | compaction (f1 f2 : MirFunction) : NoopRemoval f1 f2 → FunctionTransformed f1 f2
  | eta (f1 f2 : MirFunction) : EtaReduction f1 f2 → FunctionTransformed f1 f2
  | compose (f1 f2 f3 : MirFunction) : FunctionTransformed f1 f2 → FunctionTransformed f2 f3 → FunctionTransformed f1 f3

theorem function_transformed_preserves_semantics (f1 f2 : MirFunction)
    (h : FunctionTransformed f1 f2) :
    SemanticEquivalent f1 f2 := by
  induction h with
  | driving a b h_drive =>
    exact driving_preserves_semantics a b h_drive
  | distillation a b h_dist =>
    intro args res
    exact distillation_preserves_semantics a b h_dist args res
  | compaction a b h_comp =>
    exact noop_removal_preserves_semantics a b h_comp
  | eta a b h_eta =>
    exact eta_reduction_preserves_semantics a b h_eta
  | compose a b c _ _ ih1 ih2 =>
    exact semantic_equiv_trans ih1 ih2

inductive SupercompilerProduces : MirProgram → MirProgram → Prop where
  | pipeline (prog residual : MirProgram) :
      (∀ f1 ∈ prog.functions, ∀ f2 ∈ residual.functions, f1.name = f2.name → FunctionTransformed f1 f2) →
      SupercompilerProduces prog residual

theorem compose_all_proofs {prog residual : MirProgram}
    (h : SupercompilerProduces prog residual) :
    ∀ func residual_func,
      (func ∈ prog.functions) →
      (residual_func ∈ residual.functions) →
      func.name = residual_func.name →
      SemanticEquivalent func residual_func := by
  intro func residual_func h_in_prog h_in_res h_name
  cases h with
  | pipeline h_all =>
    have h_trans := h_all func h_in_prog residual_func h_in_res h_name
    exact function_transformed_preserves_semantics func residual_func h_trans

/-- The full numlang supercompiler pipeline is semantics-preserving.
    For any MirProgram `prog` and its supercompiled residual `residual`:
    every function pair (f, f') satisfies SemanticEquivalent f f'. -/
theorem supercompiler_sound
    (prog : MirProgram) (residual : MirProgram)
    (h : SupercompilerProduces prog residual) :
    ∀ func residual_func,
      (func ∈ prog.functions) →
      (residual_func ∈ residual.functions) →
      func.name = residual_func.name →
      SemanticEquivalent func residual_func := by
  exact compose_all_proofs h

end Supercompiler
