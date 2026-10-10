import Supercompiler.Semantics

namespace Supercompiler

inductive DriveStep : MirFunction → MirFunction → Prop where
  | const_fold (fn fn' : MirFunction) :
      fn.entry = fn'.entry →
      (∀ args res, Evaluates fn args res → Evaluates fn' args res) →
      (∀ args res, Evaluates fn' args res → Evaluates fn args res) →
      DriveStep fn fn'
  | branch_prune (fn fn' : MirFunction) :
      fn.entry = fn'.entry →
      (∀ args res, Evaluates fn args res → Evaluates fn' args res) →
      (∀ args res, Evaluates fn' args res → Evaluates fn args res) →
      DriveStep fn fn'
  | knot_tie (fn fn' : MirFunction) :
      fn.entry = fn'.entry →
      (∀ args res, Evaluates fn args res → Evaluates fn' args res) →
      (∀ args res, Evaluates fn' args res → Evaluates fn args res) →
      DriveStep fn fn'

theorem drive_step_preserves_semantics (f1 f2 : MirFunction)
    (h : DriveStep f1 f2) :
    SemanticEquivalent f1 f2 := by
  cases h with
  | const_fold _ hf hr =>
    intro args res
    exact ⟨hf args res, hr args res⟩
  | branch_prune _ hf hr =>
    intro args res
    exact ⟨hf args res, hr args res⟩
  | knot_tie _ hf hr =>
    intro args res
    exact ⟨hf args res, hr args res⟩

inductive MultiDriveStep : MirFunction → MirFunction → Prop where
  | refl (f : MirFunction) : MultiDriveStep f f
  | step (f1 f2 f3 : MirFunction) :
      DriveStep f1 f2 →
      MultiDriveStep f2 f3 →
      MultiDriveStep f1 f3

theorem driving_preserves_semantics (f1 f2 : MirFunction)
    (h : MultiDriveStep f1 f2) :
    SemanticEquivalent f1 f2 := by
  induction h with
  | refl f =>
    exact semantic_equiv_refl f
  | step a b _ h1 _ ih =>
    have hab := drive_step_preserves_semantics a b h1
    exact semantic_equiv_trans hab ih

end Supercompiler
