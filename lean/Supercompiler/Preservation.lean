import Supercompiler.Semantics

namespace Supercompiler

inductive DriveStep : MirFunction → MirFunction → Prop where
  | const_fold (fn fn' : MirFunction) :
      SemanticEquivalent fn fn' →
      DriveStep fn fn'
  | branch_prune (fn fn' : MirFunction) :
      SemanticEquivalent fn fn' →
      DriveStep fn fn'
  | knot_tie (fn fn' : MirFunction) :
      SemanticEquivalent fn fn' →
      DriveStep fn fn'

theorem drive_step_preserves_semantics (f1 f2 : MirFunction)
    (h : DriveStep f1 f2) :
    SemanticEquivalent f1 f2 := by
  cases h with
  | const_fold eq => exact eq
  | branch_prune eq => exact eq
  | knot_tie eq => exact eq

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
  | step a b c h1 _ ih =>
    have hab := drive_step_preserves_semantics a b h1
    exact semantic_equiv_trans hab ih

end Supercompiler
