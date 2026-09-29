import Supercompiler.Semantics

namespace Supercompiler

theorem mrsc_selection_preserves_semantics
    (func : MirFunction)
    (lattice : List MirFunction)
    (h_lattice : ∀ candidate ∈ lattice, SemanticEquivalent func candidate)
    (selected : MirFunction)
    (h_selected : selected ∈ lattice) :
    SemanticEquivalent func selected := by
  exact h_lattice selected h_selected

end Supercompiler
