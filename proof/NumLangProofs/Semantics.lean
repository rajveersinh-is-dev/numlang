namespace NumLang

inductive Op
  | add | sub | mul | div | eq | lt
  deriving Repr, DecidableEq

inductive Val
  | intVal : Int → Val
  | ptrVal : Nat → Val
  deriving Repr, DecidableEq

inductive Expr
  | lit : Int → Expr
  | var : String → Expr
  | bin : Op → Expr → Expr → Expr
  | cond : Expr → Expr → Expr → Expr
  | letIn : String → Expr → Expr → Expr
  | call : String → Expr → Expr
  | box : Expr → Expr
  | deref : Expr → Expr
  | assign : Expr → Expr → Expr
  deriving Repr, DecidableEq

structure FunctionDef where
  param : String
  body : Expr
  deriving Repr, DecidableEq

abbrev FunEnv := String → Option FunctionDef

abbrev Env := List (String × Val)

abbrev Heap := List Val

def lookup (env : Env) (x : String) : Option Val :=
  match env with
  | [] => none
  | (y, v) :: rest => if x = y then some v else lookup rest x

def evalOp (op : Op) (v1 v2 : Int) : Option Int :=
  match op with
  | Op.add => some (v1 + v2)
  | Op.sub => some (v1 - v2)
  | Op.mul => some (v1 * v2)
  | Op.div => if v2 = 0 then none else some (v1 / v2)
  | Op.eq  => some (if v1 = v2 then 1 else 0)
  | Op.lt  => some (if v1 < v2 then 1 else 0)

inductive BigStep : FunEnv → Env → Heap → Expr → Val → Heap → Prop where
  | lit (E : FunEnv) (env : Env) (h : Heap) (n : Int) :
      BigStep E env h (Expr.lit n) (Val.intVal n) h
  | var (E : FunEnv) (env : Env) (h : Heap) (x : String) (v : Val) :
      lookup env x = some v →
      BigStep E env h (Expr.var x) v h
  | bin (E : FunEnv) (env : Env) (h h1 h2 : Heap) (op : Op) (e1 e2 : Expr) (n1 n2 n3 : Int) :
      BigStep E env h e1 (Val.intVal n1) h1 →
      BigStep E env h1 e2 (Val.intVal n2) h2 →
      evalOp op n1 n2 = some n3 →
      BigStep E env h (Expr.bin op e1 e2) (Val.intVal n3) h2
  | cond_true (E : FunEnv) (env : Env) (h h1 h2 : Heap) (c t f : Expr) (v : Val) (nc : Int) :
      BigStep E env h c (Val.intVal nc) h1 →
      nc ≠ 0 →
      BigStep E env h1 t v h2 →
      BigStep E env h (Expr.cond c t f) v h2
  | cond_false (E : FunEnv) (env : Env) (h h1 h2 : Heap) (c t f : Expr) (v : Val) :
      BigStep E env h c (Val.intVal 0) h1 →
      BigStep E env h1 f v h2 →
      BigStep E env h (Expr.cond c t f) v h2
  | letIn (E : FunEnv) (env : Env) (h h1 h2 : Heap) (x : String) (valExpr body : Expr) (vVal vBody : Val) :
      BigStep E env h valExpr vVal h1 →
      BigStep E ((x, vVal) :: env) h1 body vBody h2 →
      BigStep E env h (Expr.letIn x valExpr body) vBody h2
  | call (E : FunEnv) (env : Env) (h h1 h2 : Heap) (f : String) (arg : Expr) (vArg vRet : Val) (fdef : FunctionDef) :
      BigStep E env h arg vArg h1 →
      E f = some fdef →
      BigStep E [(fdef.param, vArg)] h1 fdef.body vRet h2 →
      BigStep E env h (Expr.call f arg) vRet h2
  | box (E : FunEnv) (env : Env) (h h1 : Heap) (e : Expr) (v : Val) :
      BigStep E env h e v h1 →
      BigStep E env h (Expr.box e) (Val.ptrVal h1.length) (h1 ++ [v])
  | deref (E : FunEnv) (env : Env) (h h1 : Heap) (e : Expr) (loc : Nat) (v : Val) :
      BigStep E env h e (Val.ptrVal loc) h1 →
      h1[loc]? = some v →
      BigStep E env h (Expr.deref e) v h1
  | assign (E : FunEnv) (env : Env) (h h1 h2 : Heap) (ptrE valE : Expr) (loc : Nat) (v : Val) :
      BigStep E env h ptrE (Val.ptrVal loc) h1 →
      BigStep E env h1 valE v h2 →
      loc < h2.length →
      BigStep E env h (Expr.assign ptrE valE) v (h2.set loc v)

theorem bigStep_lit_inv (E : FunEnv) (env : Env) (h : Heap) (n : Int) (v : Val) (h' : Heap)
    (H : BigStep E env h (Expr.lit n) v h') : v = Val.intVal n ∧ h' = h := by
  cases H
  exact ⟨rfl, rfl⟩

end NumLang
