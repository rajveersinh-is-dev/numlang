import Supercompiler.Semantics
import Supercompiler.Preservation
import Supercompiler.Distillation
import Supercompiler.MRSC
import Supercompiler.Refinement
import Supercompiler.Compaction

namespace Supercompiler

inductive SupercompilerProduces : MirProgram → MirProgram → Prop where
  | pipeline (prog residual : MirProgram) :
      (∀ f1 ∈ prog.functions, ∀ f2 ∈ residual.functions, f1.name = f2.name → SemanticEquivalent f1 f2) →
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
    exact h_all func h_in_prog residual_func h_in_res h_name

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
