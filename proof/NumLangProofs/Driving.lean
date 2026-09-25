import NumLangProofs.Semantics

namespace NumLang

/-- Single-step driving reduction on Core Expr.
    Performs constant folding and deterministic branch selection on static redexes. -/
def drive : Expr → Option Expr
  | Expr.bin op (Expr.lit n1) (Expr.lit n2) =>
    match evalOp op n1 n2 with
    | some n3 => some (Expr.lit n3)
    | none => none
  | Expr.cond (Expr.lit nc) t f =>
    if nc ≠ 0 then some t else some f
  | _ => none

/-- Inversion lemma: A literal evaluates only to its own integer value. -/
theorem bigStep_lit_inv (env : Env) (n : Int) (v : Val) (h : BigStep env (Expr.lit n) v) :
    v = Val.intVal n := by
  cases h
  rfl

/-- Theorem: Driving preserves big-step operational semantics. -/
theorem driving_preserves_semantics :
    ∀ (env : Env) (e : Expr) (v : Val),
      BigStep env e v →
      ∀ (res : Expr), drive e = some res →
      BigStep env res v := by
  intro env e v h
  induction h with
  | lit env n =>
    intro res hd
    nomatch hd
  | var env idx val hget =>
    intro res hd
    nomatch hd
  | bin env op e1 e2 n1 n2 n3 h1 h2 hop ih1 ih2 =>
    intro res hd
    cases e1 with
    | lit m1 =>
      cases e2 with
      | lit m2 =>
        have hm1 : Val.intVal n1 = Val.intVal m1 := bigStep_lit_inv env m1 (Val.intVal n1) h1
        have hm2 : Val.intVal n2 = Val.intVal m2 := bigStep_lit_inv env m2 (Val.intVal n2) h2
        injection hm1 with eq1
        injection hm2 with eq2
        subst eq1 eq2
        dsimp [drive] at hd
        rw [hop] at hd
        injection hd with hres
        subst hres
        exact BigStep.lit env n3
      | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ =>
        nomatch hd
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ =>
      nomatch hd
  | cond_true env c t f val nc hc hnc ht ihc _ =>
    intro res hd
    cases c with
    | lit mc =>
      have hmc : Val.intVal nc = Val.intVal mc := bigStep_lit_inv env mc (Val.intVal nc) hc
      injection hmc with eqc
      subst eqc
      dsimp [drive] at hd
      split at hd
      · rename_i _
        injection hd with hres
        subst hres
        exact ht
      · rename_i hneg
        exact False.elim (hneg hnc)
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ =>
      nomatch hd
  | cond_false env c t f val hc hf ihc _ =>
    intro res hd
    cases c with
    | lit mc =>
      have hmc : Val.intVal 0 = Val.intVal mc := bigStep_lit_inv env mc (Val.intVal 0) hc
      injection hmc with eqc
      subst eqc
      dsimp [drive] at hd
      injection hd with hres
      subst hres
      exact hf
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ =>
      nomatch hd
  | letIn env valExpr body vVal vBody hval hbody ihval ihbody =>
    intro res hd
    nomatch hd

end NumLang
