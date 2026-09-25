namespace NumLang

inductive Op
  | add | sub | mul | div | eq | lt
  deriving Repr, DecidableEq

inductive Expr
  | lit : Int → Expr
  | var : Nat → Expr
  | bin : Op → Expr → Expr → Expr
  | cond : Expr → Expr → Expr → Expr
  | letIn : Expr → Expr → Expr
  deriving Repr, DecidableEq

inductive Val
  | intVal : Int → Val
  deriving Repr, DecidableEq

abbrev Env := List Val

def evalOp (op : Op) (v1 v2 : Int) : Option Int :=
  match op with
  | Op.add => some (v1 + v2)
  | Op.sub => some (v1 - v2)
  | Op.mul => some (v1 * v2)
  | Op.div => if v2 = 0 then none else some (v1 / v2)
  | Op.eq  => some (if v1 = v2 then 1 else 0)
  | Op.lt  => some (if v1 < v2 then 1 else 0)

inductive BigStep : Env → Expr → Val → Prop where
  | lit (env : Env) (n : Int) :
      BigStep env (Expr.lit n) (Val.intVal n)
  | var (env : Env) (idx : Nat) (v : Val) :
      env[idx]? = some v →
      BigStep env (Expr.var idx) v
  | bin (env : Env) (op : Op) (e1 e2 : Expr) (n1 n2 n3 : Int) :
      BigStep env e1 (Val.intVal n1) →
      BigStep env e2 (Val.intVal n2) →
      evalOp op n1 n2 = some n3 →
      BigStep env (Expr.bin op e1 e2) (Val.intVal n3)
  | cond_true (env : Env) (c t f : Expr) (v : Val) (nc : Int) :
      BigStep env c (Val.intVal nc) →
      nc ≠ 0 →
      BigStep env t v →
      BigStep env (Expr.cond c t f) v
  | cond_false (env : Env) (c t f : Expr) (v : Val) :
      BigStep env c (Val.intVal 0) →
      BigStep env f v →
      BigStep env (Expr.cond c t f) v
  | letIn (env : Env) (valExpr body : Expr) (vVal vBody : Val) :
      BigStep env valExpr vVal →
      BigStep (vVal :: env) body vBody →
      BigStep env (Expr.letIn valExpr body) vBody

end NumLang
