import NumLangProofs.Semantics

namespace NumLang

/-- Single-step driving reduction on Core Expr.
    Performs constant folding on binary operations and deterministic branch selection
    on conditionals with static redexes. -/
def drive (_E : FunEnv) : Expr → Option Expr
  | Expr.bin op (Expr.lit n1) (Expr.lit n2) =>
    match evalOp op n1 n2 with
    | some n3 => some (Expr.lit n3)
    | none => none
  | Expr.cond (Expr.lit nc) t f =>
    if nc ≠ 0 then some t else some f
  | _ => none

/-- Supercompiler loop folding: folds a configuration into a recursive call to a specialized function. -/
def foldCall (f : String) (arg : Expr) : Expr :=
  Expr.call f arg

/-- Supercompiler Most Specific Generalization (MSG): generalizes a subterm into a let-binding. -/
def generalize (x : String) (a : Expr) (body : Expr) : Expr :=
  Expr.letIn x a body

/-- Theorem: Driving preserves big-step operational semantics (forward direction). -/
theorem driving_correctness (E : FunEnv) (env : Env) (h : Heap) (e : Expr) (v : Val) (h' : Heap)
    (he : BigStep E env h e v h') (res : Expr) (hd : drive E e = some res) :
    BigStep E env h res v h' := by
  cases e with
  | bin op e1 e2 =>
    cases e1 with
    | lit n1 =>
      cases e2 with
      | lit n2 =>
        dsimp [drive] at hd
        cases he
        rename_i h1 m1 m2 m3 hop he1 he2
        have h_inv1 := bigStep_lit_inv E env h n1 (Val.intVal m1) h1 he1
        have h_inv2 := bigStep_lit_inv E env h1 n2 (Val.intVal m2) h' he2
        rcases h_inv1 with ⟨eq_v1, eq_h1⟩
        rcases h_inv2 with ⟨eq_v2, eq_h2⟩
        injection eq_v1 with eq1
        injection eq_v2 with eq2
        subst eq1 eq2 eq_h1 eq_h2
        rw [hop] at hd
        injection hd with h_res
        subst h_res
        exact BigStep.lit E env h' m3
      | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
        nomatch hd
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
      nomatch hd
  | cond c t f =>
    cases c with
    | lit nc =>
      dsimp [drive] at hd
      cases he
      · rename_i h1 mc hne hc ht
        have h_inv := bigStep_lit_inv E env h nc (Val.intVal mc) h1 hc
        rcases h_inv with ⟨eq_v, eq_h⟩
        injection eq_v with eq_mc
        subst eq_mc eq_h
        split at hd
        · rename_i _
          injection hd with h_res
          subst h_res
          exact ht
        · rename_i h_not
          exact False.elim (h_not hne)
      · rename_i h1 hc hf
        have h_inv := bigStep_lit_inv E env h nc (Val.intVal 0) h1 hc
        rcases h_inv with ⟨eq_v, eq_h⟩
        injection eq_v with eq_mc
        subst eq_mc eq_h
        split at hd
        · rename_i h_not
          exact False.elim (h_not rfl)
        · rename_i _
          injection hd with h_res
          subst h_res
          exact hf
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
      nomatch hd
  | lit _ | var _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
    nomatch hd

/-- Theorem: Driving preserves big-step operational semantics (reverse direction / soundness). -/
theorem driving_soundness (E : FunEnv) (env : Env) (h : Heap) (e res : Expr) (v : Val) (h' : Heap)
    (hd : drive E e = some res) (hres : BigStep E env h res v h') :
    BigStep E env h e v h' := by
  cases e with
  | bin op e1 e2 =>
    cases e1 with
    | lit n1 =>
      cases e2 with
      | lit n2 =>
        dsimp [drive] at hd
        cases h_eval : evalOp op n1 n2 with
        | none =>
          rw [h_eval] at hd
          nomatch hd
        | some n3 =>
          rw [h_eval] at hd
          injection hd with h_eq
          subst h_eq
          have h_inv := bigStep_lit_inv E env h n3 v h' hres
          rcases h_inv with ⟨eq_v, eq_h⟩
          subst eq_v eq_h
          apply BigStep.bin (h1 := h') (n1 := n1) (n2 := n2) (n3 := n3)
          · exact BigStep.lit E env h' n1
          · exact BigStep.lit E env h' n2
          · exact h_eval
      | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
        nomatch hd
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
      nomatch hd
  | cond c t f =>
    cases c with
    | lit nc =>
      dsimp [drive] at hd
      by_cases hnc : nc = 0
      · subst hnc
        dsimp at hd
        injection hd with h_eq
        subst h_eq
        apply BigStep.cond_false
        · exact BigStep.lit E env h 0
        · exact hres
      · have hne : nc ≠ 0 := hnc
        split at hd
        · rename_i _
          injection hd with h_eq
          subst h_eq
          apply BigStep.cond_true (nc := nc)
          · exact BigStep.lit E env h nc
          · exact hne
          · exact hres
        · rename_i h_contra
          exact False.elim (h_contra hne)
    | var _ | bin _ _ _ | cond _ _ _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
      nomatch hd
  | lit _ | var _ | letIn _ _ _ | call _ _ | box _ | deref _ | assign _ _ =>
    nomatch hd

/-- Semantic Preservation Theorem 1: Driving preserves big-step evaluation equivalence. -/
theorem driving_equiv (E : FunEnv) (env : Env) (h : Heap) (e res : Expr) (v : Val) (h' : Heap)
    (hd : drive E e = some res) :
    BigStep E env h e v h' ↔ BigStep E env h res v h' := by
  constructor
  · intro he
    exact driving_correctness E env h e v h' he res hd
  · intro hres
    exact driving_soundness E env h e res v h' hd hres

/-- Supercompiler loop folding correctness: evaluating the specialized function body implies
    the folded call evaluates to the same result. -/
theorem folding_correctness (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vArg vRet : Val) (h1 h2 : Heap)
    (harg : BigStep E env h arg vArg h1)
    (hE : E f = some fdef)
    (hbody : BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2) :
    BigStep E env h (foldCall f arg) vRet h2 :=
  BigStep.call E env h h1 h2 f arg vArg vRet fdef harg hE hbody

/-- Supercompiler loop folding soundness: if a folded call evaluates, then there exist
    argument evaluation steps leading to the function body evaluation. -/
theorem folding_soundness (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vRet : Val) (h2 : Heap)
    (hE : E f = some fdef)
    (hcall : BigStep E env h (foldCall f arg) vRet h2) :
    ∃ (vArg : Val) (h1 : Heap), BigStep E env h arg vArg h1 ∧ BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2 := by
  cases hcall
  rename_i h1 vArg fdef' hEf harg hbody
  rw [hE] at hEf
  injection hEf with eq_fdef
  subst eq_fdef
  exact ⟨vArg, h1, harg, hbody⟩

/-- Semantic Preservation Theorem 2: Supercompiler loop folding preserves big-step operational semantics.
    A folded call is semantically equivalent to argument evaluation followed by body evaluation. -/
theorem folding_equiv (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vRet : Val) (h2 : Heap)
    (hE : E f = some fdef) :
    BigStep E env h (foldCall f arg) vRet h2 ↔
    ∃ (vArg : Val) (h1 : Heap), BigStep E env h arg vArg h1 ∧ BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2 := by
  constructor
  · exact folding_soundness E env h f arg fdef vRet h2 hE
  · intro ⟨vArg, h1, ha, hb⟩
    exact folding_correctness E env h f arg fdef vArg vRet h1 h2 ha hE hb

/-- Supercompiler Most Specific Generalization (let-expression abstraction) correctness. -/
theorem generalization_correctness (E : FunEnv) (env : Env) (h : Heap)
    (x : String) (a body : Expr) (va : Val) (h1 : Heap) (v : Val) (h2 : Heap)
    (ha : BigStep E env h a va h1)
    (hbody : BigStep E ((x, va) :: env) h1 body v h2) :
    BigStep E env h (generalize x a body) v h2 :=
  BigStep.letIn E env h h1 h2 x a body va v ha hbody

/-- Supercompiler Most Specific Generalization (let-expression abstraction) soundness. -/
theorem generalization_soundness (E : FunEnv) (env : Env) (h : Heap)
    (x : String) (a body : Expr) (v : Val) (h2 : Heap)
    (hgen : BigStep E env h (generalize x a body) v h2) :
    ∃ (va : Val) (h1 : Heap), BigStep E env h a va h1 ∧ BigStep E ((x, va) :: env) h1 body v h2 := by
  cases hgen
  rename_i h1 va hval hbody
  exact ⟨va, h1, hval, hbody⟩

/-- Semantic Preservation Theorem 3: Most Specific Generalization
    preserves big-step operational semantics. -/
theorem generalization_equiv (E : FunEnv) (env : Env) (h : Heap)
    (x : String) (a body : Expr) (v : Val) (h2 : Heap) :
    BigStep E env h (generalize x a body) v h2 ↔
    ∃ (va : Val) (h1 : Heap), BigStep E env h a va h1 ∧ BigStep E ((x, va) :: env) h1 body v h2 := by
  constructor
  · exact generalization_soundness E env h x a body v h2
  · intro ⟨va, h1, ha, hb⟩
    exact generalization_correctness E env h x a body va h1 v h2 ha hb

/-- One unfolding (driving) step of a recursive function call redex. -/
def unfoldCall (_f : String) (arg : Expr) (fdef : FunctionDef) : Expr :=
  Expr.letIn fdef.param arg fdef.body

/-- Function call unfolding correctness: unfolding a defined function call preserves big-step evaluation. -/
theorem unfold_call_correctness (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vArg vRet : Val) (h1 h2 : Heap)
    (harg : BigStep E env h arg vArg h1)
    (hE : E f = some fdef)
    (hbody : BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2) :
    BigStep E env h (Expr.call f arg) vRet h2 :=
  BigStep.call E env h h1 h2 f arg vArg vRet fdef harg hE hbody

/-- Function call unfolding soundness: any evaluating call can be unfolded to its argument and body evaluation. -/
theorem unfold_call_soundness (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vRet : Val) (h2 : Heap)
    (hE : E f = some fdef)
    (hcall : BigStep E env h (Expr.call f arg) vRet h2) :
    ∃ (vArg : Val) (h1 : Heap), BigStep E env h arg vArg h1 ∧ BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2 := by
  cases hcall
  rename_i h1 vArg fdef' hEf harg hbody
  rw [hE] at hEf
  injection hEf with eq_fdef
  subst eq_fdef
  exact ⟨vArg, h1, harg, hbody⟩

/-- Semantic Preservation Theorem 4: One-step recursive call unfolding preserves operational semantics. -/
theorem unfold_call_equiv (E : FunEnv) (env : Env) (h : Heap)
    (f : String) (arg : Expr) (fdef : FunctionDef) (vRet : Val) (h2 : Heap)
    (hE : E f = some fdef) :
    BigStep E env h (Expr.call f arg) vRet h2 ↔
    ∃ (vArg : Val) (h1 : Heap), BigStep E env h arg vArg h1 ∧ BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2 := by
  constructor
  · exact unfold_call_soundness E env h f arg fdef vRet h2 hE
  · intro ⟨vArg, h1, ha, hb⟩
    exact unfold_call_correctness E env h f arg fdef vArg vRet h1 h2 ha hE hb

/-- Small-Step Constant Folding Preservation: binary operator folding is a valid small-step reduction. -/
theorem smallstep_const_fold (E : FunEnv) (op : Op) (n1 n2 n3 : Int)
    (hop : evalOp op n1 n2 = some n3) :
    SmallStep E (Expr.bin op (Expr.lit n1) (Expr.lit n2)) (Expr.lit n3) :=
  SmallStep.bin_redex E op n1 n2 n3 hop

/-- Small-Step Branch Pruning Preservation (True branch): static non-zero condition steps to true branch. -/
theorem smallstep_branch_prune_true (E : FunEnv) (nc : Int) (h : nc ≠ 0) (t f : Expr) :
    SmallStep E (Expr.cond (Expr.lit nc) t f) t :=
  SmallStep.cond_true E nc t f h

/-- Small-Step Branch Pruning Preservation (False branch): static zero condition steps to false branch. -/
theorem smallstep_branch_prune_false (E : FunEnv) (t f : Expr) :
    SmallStep E (Expr.cond (Expr.lit 0) t f) f :=
  SmallStep.cond_false E t f

/-- Small-Step Call Unfolding Preservation: call with constant argument steps to let-binding. -/
theorem smallstep_call_unfold (E : FunEnv) (f : String) (n : Int) (fdef : FunctionDef)
    (hE : E f = some fdef) :
    SmallStep E (Expr.call f (Expr.lit n)) (Expr.letIn fdef.param (Expr.lit n) fdef.body) :=
  SmallStep.call_unfold E f n fdef hE

end NumLang
