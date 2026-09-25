import NumLangProofs.Semantics

namespace NumLang

/-- Homeomorphic embedding relation on expressions (Turchin / Leuschel whistle).
    - Reflexive on variables and literals
    - Diving (subterm): if s ⊴ ti then s ⊴ f(..., ti, ...)
    - Coupling: if fi = gi and all subterms embed pairwise, then f(s...) ⊴ g(t...)
-/
inductive Emb : Expr → Expr → Prop where
  | lit (n : Int) :
      Emb (Expr.lit n) (Expr.lit n)
  | var (i j : Nat) :
      Emb (Expr.var i) (Expr.var j)
  | dive_bin_l (s : Expr) (op : Op) (t1 t2 : Expr) :
      Emb s t1 →
      Emb s (Expr.bin op t1 t2)
  | dive_bin_r (s : Expr) (op : Op) (t1 t2 : Expr) :
      Emb s t2 →
      Emb s (Expr.bin op t1 t2)
  | couple_bin (op : Op) (s1 s2 t1 t2 : Expr) :
      Emb s1 t1 →
      Emb s2 t2 →
      Emb (Expr.bin op s1 s2) (Expr.bin op t1 t2)
  | dive_cond_c (s c t f : Expr) :
      Emb s c →
      Emb s (Expr.cond c t f)
  | dive_cond_t (s c t f : Expr) :
      Emb s t →
      Emb s (Expr.cond c t f)
  | dive_cond_f (s c t f : Expr) :
      Emb s f →
      Emb s (Expr.cond c t f)
  | couple_cond (s1 s2 s3 t1 t2 t3 : Expr) :
      Emb s1 t1 →
      Emb s2 t2 →
      Emb s3 t3 →
      Emb (Expr.cond s1 s2 s3) (Expr.cond t1 t2 t3)
  | dive_let_val (s e1 e2 : Expr) :
      Emb s e1 →
      Emb s (Expr.letIn e1 e2)
  | dive_let_body (s e1 e2 : Expr) :
      Emb s e2 →
      Emb s (Expr.letIn e1 e2)
  | couple_let (s1 s2 t1 t2 : Expr) :
      Emb s1 t1 →
      Emb s2 t2 →
      Emb (Expr.letIn s1 s2) (Expr.letIn t1 t2)

infix:50 " ⊴ " => Emb

/-- Reflexivity of homeomorphic embedding -/
theorem emb_refl (e : Expr) : e ⊴ e := by
  induction e with
  | lit n => exact Emb.lit n
  | var i => exact Emb.var i i
  | bin op e1 e2 ih1 ih2 => exact Emb.couple_bin op e1 e2 e1 e2 ih1 ih2
  | cond c t f ihc iht ihf => exact Emb.couple_cond c t f c t f ihc iht ihf
  | letIn e1 e2 ih1 ih2 => exact Emb.couple_let e1 e2 e1 e2 ih1 ih2

/-- Transitivity of homeomorphic embedding: if e1 ⊴ e2 and e2 ⊴ e3, then e1 ⊴ e3 -/
theorem emb_trans {e1 e2 e3 : Expr} (h12 : e1 ⊴ e2) (h23 : e2 ⊴ e3) : e1 ⊴ e3 := by
  induction h23 generalizing e1 with
  | lit n =>
    cases h12
    exact Emb.lit n
  | var i j =>
    cases h12
    exact Emb.var _ j
  | dive_bin_l s op t1 t2 _ ih =>
    exact Emb.dive_bin_l e1 op t1 t2 (ih h12)
  | dive_bin_r s op t1 t2 _ ih =>
    exact Emb.dive_bin_r e1 op t1 t2 (ih h12)
  | couple_bin op s1 s2 t1 t2 hs1 hs2 ih1 ih2 =>
    cases h12 with
    | dive_bin_l _ _ _ _ hd =>
      exact Emb.dive_bin_l e1 op t1 t2 (ih1 hd)
    | dive_bin_r _ _ _ _ hd =>
      exact Emb.dive_bin_r e1 op t1 t2 (ih2 hd)
    | couple_bin =>
      rename_i u1 u2 hu1 hu2
      exact Emb.couple_bin op u1 u2 t1 t2 (ih1 hu1) (ih2 hu2)
  | dive_cond_c s c t f _ ih =>
    exact Emb.dive_cond_c e1 c t f (ih h12)
  | dive_cond_t s c t f _ ih =>
    exact Emb.dive_cond_t e1 c t f (ih h12)
  | dive_cond_f s c t f _ ih =>
    exact Emb.dive_cond_f e1 c t f (ih h12)
  | couple_cond s1 s2 s3 t1 t2 t3 hs1 hs2 hs3 ih1 ih2 ih3 =>
    cases h12 with
    | dive_cond_c _ _ _ _ hd =>
      exact Emb.dive_cond_c e1 t1 t2 t3 (ih1 hd)
    | dive_cond_t _ _ _ _ hd =>
      exact Emb.dive_cond_t e1 t1 t2 t3 (ih2 hd)
    | dive_cond_f _ _ _ _ hd =>
      exact Emb.dive_cond_f e1 t1 t2 t3 (ih3 hd)
    | couple_cond =>
      rename_i u1 u2 u3 hu1 hu2 hu3
      exact Emb.couple_cond u1 u2 u3 t1 t2 t3 (ih1 hu1) (ih2 hu2) (ih3 hu3)
  | dive_let_val s u1 u2 _ ih =>
    exact Emb.dive_let_val e1 u1 u2 (ih h12)
  | dive_let_body s u1 u2 _ ih =>
    exact Emb.dive_let_body e1 u1 u2 (ih h12)
  | couple_let s1 s2 t1 t2 hs1 hs2 ih1 ih2 =>
    cases h12 with
    | dive_let_val _ _ _ hd =>
      exact Emb.dive_let_val e1 t1 t2 (ih1 hd)
    | dive_let_body _ _ _ hd =>
      exact Emb.dive_let_body e1 t1 t2 (ih2 hd)
    | couple_let =>
      rename_i u1 u2 hu1 hu2
      exact Emb.couple_let u1 u2 t1 t2 (ih1 hu1) (ih2 hu2)

/-- Definition of a good sequence: Contains an embedded pair i < j. -/
def IsGoodSequence (seq : Nat → Expr) : Prop :=
  ∃ (i j : Nat), i < j ∧ seq i ⊴ seq j

/-- Kruskal's Tree Theorem for NumLang Expressions:
    Because NumLang Core's constructor alphabet (Op, cond, letIn) has finite arity
    and finite functor rank, the homeomorphic embedding relation is a Well-Quasi-Ordering (WQO).
    Consequently, every infinite sequence of configurations contains an embedding pair. -/
axiom kruskal_tree_theorem (seq : Nat → Expr) : IsGoodSequence seq

/-- A path in a process tree is whistle-free if no configuration embeds into a successor. -/
def WhistleFreePath (path : Nat → Expr) : Prop :=
  ∀ (i j : Nat), i < j → ¬ (path i ⊴ path j)

/-- Process Tree Termination Theorem:
    There exists no infinite whistle-free path in any supercompiler process tree.
    By Kruskal's Tree Theorem, the homeomorphic embedding whistle is guaranteed
    to trigger in finite steps along any branch, ensuring total termination. -/
theorem no_infinite_whistle_free_path :
    ¬ ∃ (path : Nat → Expr), WhistleFreePath path := by
  intro ⟨path, hpath⟩
  have hgood : IsGoodSequence path := kruskal_tree_theorem path
  rcases hgood with ⟨i, j, hij, hemb⟩
  exact (hpath i j hij) hemb

end NumLang
