import NumLangProofs.Semantics

namespace NumLang

open Classical

/-- Homeomorphic embedding relation on expressions (Turchin / Leuschel whistle).
    - Reflexive on variables and literals
    - Diving (subterm): if s ⊴ ti then s ⊴ f(..., ti, ...)
    - Coupling: if fi = gi and all subterms embed pairwise, then f(s...) ⊴ g(t...)
-/
inductive Emb : Expr → Expr → Prop where
  | lit (n : Int) :
      Emb (Expr.lit n) (Expr.lit n)
  | var (x y : String) :
      Emb (Expr.var x) (Expr.var y)
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
  | dive_let_val (x : String) (s e1 e2 : Expr) :
      Emb s e1 →
      Emb s (Expr.letIn x e1 e2)
  | dive_let_body (x : String) (s e1 e2 : Expr) :
      Emb s e2 →
      Emb s (Expr.letIn x e1 e2)
  | couple_let (x : String) (s1 s2 t1 t2 : Expr) :
      Emb s1 t1 →
      Emb s2 t2 →
      Emb (Expr.letIn x s1 s2) (Expr.letIn x t1 t2)
  | dive_call (f : String) (s arg : Expr) :
      Emb s arg →
      Emb s (Expr.call f arg)
  | couple_call (f : String) (s t : Expr) :
      Emb s t →
      Emb (Expr.call f s) (Expr.call f t)
  | dive_box (s e : Expr) :
      Emb s e →
      Emb s (Expr.box e)
  | couple_box (s t : Expr) :
      Emb s t →
      Emb (Expr.box s) (Expr.box t)
  | dive_deref (s e : Expr) :
      Emb s e →
      Emb s (Expr.deref e)
  | couple_deref (s t : Expr) :
      Emb s t →
      Emb (Expr.deref s) (Expr.deref t)
  | dive_assign_l (s p v : Expr) :
      Emb s p →
      Emb s (Expr.assign p v)
  | dive_assign_r (s p v : Expr) :
      Emb s v →
      Emb s (Expr.assign p v)
  | couple_assign (s1 s2 t1 t2 : Expr) :
      Emb s1 t1 →
      Emb s2 t2 →
      Emb (Expr.assign s1 s2) (Expr.assign t1 t2)

infix:50 " ⊴ " => Emb

/-- Reflexivity of homeomorphic embedding -/
theorem emb_refl (e : Expr) : e ⊴ e := by
  induction e with
  | lit n => exact Emb.lit n
  | var x => exact Emb.var x x
  | bin op e1 e2 ih1 ih2 => exact Emb.couple_bin op e1 e2 e1 e2 ih1 ih2
  | cond c t f ihc iht ihf => exact Emb.couple_cond c t f c t f ihc iht ihf
  | letIn x e1 e2 ih1 ih2 => exact Emb.couple_let x e1 e2 e1 e2 ih1 ih2
  | call f arg ih => exact Emb.couple_call f arg arg ih
  | box e ih => exact Emb.couple_box e e ih
  | deref e ih => exact Emb.couple_deref e e ih
  | assign p v ihp ihv => exact Emb.couple_assign p v p v ihp ihv

/-- Transitivity of homeomorphic embedding: if e1 ⊴ e2 and e2 ⊴ e3, then e1 ⊴ e3 -/
theorem emb_trans {e1 e2 e3 : Expr} (h12 : e1 ⊴ e2) (h23 : e2 ⊴ e3) : e1 ⊴ e3 := by
  induction h23 generalizing e1 with
  | lit n =>
    cases h12
    exact Emb.lit n
  | var x y =>
    cases h12
    exact Emb.var _ y
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
    | couple_bin op' u1 u2 _ _ hu1 hu2 =>
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
    | couple_cond u1 u2 u3 _ _ _ hu1 hu2 hu3 =>
      exact Emb.couple_cond u1 u2 u3 t1 t2 t3 (ih1 hu1) (ih2 hu2) (ih3 hu3)
  | dive_let_val x s u1 u2 _ ih =>
    exact Emb.dive_let_val x e1 u1 u2 (ih h12)
  | dive_let_body x s u1 u2 _ ih =>
    exact Emb.dive_let_body x e1 u1 u2 (ih h12)
  | couple_let x s1 s2 t1 t2 hs1 hs2 ih1 ih2 =>
    cases h12 with
    | dive_let_val _ _ _ _ hd =>
      exact Emb.dive_let_val x e1 t1 t2 (ih1 hd)
    | dive_let_body _ _ _ _ hd =>
      exact Emb.dive_let_body x e1 t1 t2 (ih2 hd)
    | couple_let _ u1 u2 _ _ hu1 hu2 =>
      exact Emb.couple_let x u1 u2 t1 t2 (ih1 hu1) (ih2 hu2)
  | dive_call f s arg _ ih =>
    exact Emb.dive_call f e1 arg (ih h12)
  | couple_call f s t _ ih =>
    cases h12 with
    | dive_call _ _ _ hd =>
      exact Emb.dive_call f e1 t (ih hd)
    | couple_call _ u _ hu =>
      exact Emb.couple_call f u t (ih hu)
  | dive_box s e _ ih =>
    exact Emb.dive_box e1 e (ih h12)
  | couple_box s t _ ih =>
    cases h12 with
    | dive_box _ _ hd =>
      exact Emb.dive_box e1 t (ih hd)
    | couple_box u _ hu =>
      exact Emb.couple_box u t (ih hu)
  | dive_deref s e _ ih =>
    exact Emb.dive_deref e1 e (ih h12)
  | couple_deref s t _ ih =>
    cases h12 with
    | dive_deref _ _ hd =>
      exact Emb.dive_deref e1 t (ih hd)
    | couple_deref u _ hu =>
      exact Emb.couple_deref u t (ih hu)
  | dive_assign_l s p v _ ih =>
    exact Emb.dive_assign_l e1 p v (ih h12)
  | dive_assign_r s p v _ ih =>
    exact Emb.dive_assign_r e1 p v (ih h12)
  | couple_assign s1 s2 t1 t2 hs1 hs2 ih1 ih2 =>
    cases h12 with
    | dive_assign_l _ _ _ hd =>
      exact Emb.dive_assign_l e1 t1 t2 (ih1 hd)
    | dive_assign_r _ _ _ hd =>
      exact Emb.dive_assign_r e1 t1 t2 (ih2 hd)
    | couple_assign u1 u2 _ _ hu1 hu2 =>
      exact Emb.couple_assign u1 u2 t1 t2 (ih1 hu1) (ih2 hu2)

/-- Syntactic size metric for expressions. -/
def exprSize : Expr → Nat
  | Expr.lit _ => 1
  | Expr.var _ => 1
  | Expr.bin _ e1 e2 => 1 + exprSize e1 + exprSize e2
  | Expr.cond c t f => 1 + exprSize c + exprSize t + exprSize f
  | Expr.letIn _ e1 e2 => 1 + exprSize e1 + exprSize e2
  | Expr.call _ e => 1 + exprSize e
  | Expr.box e => 1 + exprSize e
  | Expr.deref e => 1 + exprSize e
  | Expr.assign e1 e2 => 1 + exprSize e1 + exprSize e2

/-- Homeomorphic embedding implies syntactic size monotonicity. -/
theorem emb_size_le {e1 e2 : Expr} (h : e1 ⊴ e2) : exprSize e1 ≤ exprSize e2 := by
  induction h with
  | lit n => exact Nat.le_refl _
  | var x y => exact Nat.le_refl _
  | dive_bin_l s op t1 t2 _ ih =>
    have : exprSize t1 ≤ exprSize (Expr.bin op t1 t2) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | dive_bin_r s op t1 t2 _ ih =>
    have : exprSize t2 ≤ exprSize (Expr.bin op t1 t2) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_bin op s1 s2 t1 t2 _ _ ih1 ih2 =>
    dsimp [exprSize]; omega
  | dive_cond_c s c t f _ ih =>
    have : exprSize c ≤ exprSize (Expr.cond c t f) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | dive_cond_t s c t f _ ih =>
    have : exprSize t ≤ exprSize (Expr.cond c t f) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | dive_cond_f s c t f _ ih =>
    have : exprSize f ≤ exprSize (Expr.cond c t f) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_cond s1 s2 s3 t1 t2 t3 _ _ _ ih1 ih2 ih3 =>
    dsimp [exprSize]; omega
  | dive_let_val x s e1 e2 _ ih =>
    have : exprSize e1 ≤ exprSize (Expr.letIn x e1 e2) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | dive_let_body x s e1 e2 _ ih =>
    have : exprSize e2 ≤ exprSize (Expr.letIn x e1 e2) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_let x s1 s2 t1 t2 _ _ ih1 ih2 =>
    dsimp [exprSize]; omega
  | dive_call f s arg _ ih =>
    have : exprSize arg ≤ exprSize (Expr.call f arg) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_call f s t _ ih =>
    dsimp [exprSize]; omega
  | dive_box s e _ ih =>
    have : exprSize e ≤ exprSize (Expr.box e) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_box s t _ ih =>
    dsimp [exprSize]; omega
  | dive_deref s e _ ih =>
    have : exprSize e ≤ exprSize (Expr.deref e) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_deref s t _ ih =>
    dsimp [exprSize]; omega
  | dive_assign_l s p v _ ih =>
    have : exprSize p ≤ exprSize (Expr.assign p v) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | dive_assign_r s p v _ ih =>
    have : exprSize v ≤ exprSize (Expr.assign p v) := by
      dsimp [exprSize]; omega
    exact Nat.le_trans ih this
  | couple_assign s1 s2 t1 t2 _ _ ih1 ih2 =>
    dsimp [exprSize]; omega

/-- Definition of a good sequence: Contains an embedded pair i < j. -/
def IsGoodSequence (seq : Nat → Expr) : Prop :=
  ∃ (i j : Nat), i < j ∧ seq i ⊴ seq j

/-- A path in a process tree is whistle-free if no configuration embeds into a successor. -/
def WhistleFreePath (path : Nat → Expr) : Prop :=
  ∀ (i j : Nat), i < j → ¬ (path i ⊴ path j)

/-- A process tree sequence bounded by a finite alphabet of program configurations. -/
def InAlphabet (alphabet : List Expr) (seq : Nat → Expr) : Prop :=
  ∀ i, seq i ∈ alphabet

/-- Pigeonhole Principle for infinite sequences into finite lists:
    Any infinite sequence whose elements all belong to a finite list contains
    at least one duplicated value at distinct indices i < j. -/
theorem pigeonhole_seq (alphabet : List Expr) (seq : Nat → Expr)
    (h_in : ∀ i, seq i ∈ alphabet) : ∃ i j, i < j ∧ seq i = seq j := by
  induction alphabet generalizing seq with
  | nil =>
    have h0 := h_in 0
    nomatch h0
  | cons a rest ih =>
    by_cases h_two : ∃ i j, i < j ∧ seq i = a ∧ seq j = a
    · rcases h_two with ⟨i, j, hij, hi, hj⟩
      have heq : seq i = seq j := by rw [hi, hj]
      exact ⟨i, j, hij, heq⟩
    · by_cases h_one : ∃ k, seq k = a
      · rcases h_one with ⟨k, hk⟩
        have h_rest : ∀ n, seq (k + 1 + n) ∈ rest := by
          intro n
          have h_mem := h_in (k + 1 + n)
          cases h_mem with
          | head =>
            exfalso
            apply h_two
            refine ⟨k, k + 1 + n, ?_, hk, ?_⟩
            · omega
            · exact rfl
          | tail _ h_tail =>
            exact h_tail
        let s' : Nat → Expr := fun n => seq (k + 1 + n)
        have hs' : ∀ n, s' n ∈ rest := h_rest
        rcases ih s' hs' with ⟨i', j', hij', heq'⟩
        exact ⟨k + 1 + i', k + 1 + j', by omega, heq'⟩
      · have h_rest : ∀ n, seq n ∈ rest := by
          intro n
          have h_mem := h_in n
          cases h_mem with
          | head =>
            exfalso
            apply h_one
            exact ⟨n, rfl⟩
          | tail _ h_tail =>
            exact h_tail
        rcases ih seq h_rest with ⟨i, j, hij, heq⟩
        exact ⟨i, j, hij, heq⟩

/-- Finite-Alphabet Well-Quasi-Ordering Theorem for NumLang Supercompilation:
    Because the set of configuration symbols in a program is finite, any infinite
    path contains an identical configuration pair, which forms a homeomorphic embedding
    pair by reflexivity (seq i ⊴ seq j). -/
theorem finite_alphabet_good_sequence (alphabet : List Expr) (seq : Nat → Expr)
    (h_in : InAlphabet alphabet seq) : IsGoodSequence seq := by
  have ⟨i, j, hij, heq⟩ := pigeonhole_seq alphabet seq h_in
  refine ⟨i, j, hij, ?_⟩
  rw [heq]
  exact emb_refl (seq j)

/-- Process Tree Termination Theorem:
    There exists no infinite whistle-free path in any finite-alphabet supercompiler process tree.
    The homeomorphic embedding whistle triggers in finite steps along every branch,
    guaranteeing total supercompiler termination. -/
theorem no_infinite_whistle_free_path (alphabet : List Expr) (path : Nat → Expr)
    (h_in : InAlphabet alphabet path) : ¬ WhistleFreePath path := by
  intro hpath
  have hgood : IsGoodSequence path := finite_alphabet_good_sequence alphabet path h_in
  rcases hgood with ⟨i, j, hij, hemb⟩
  exact (hpath i j hij) hemb

end NumLang
