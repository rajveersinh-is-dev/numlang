namespace Supercompiler

abbrev Local := String
abbrev BasicBlockId := Nat

inductive Op where
  | add
  | sub
  | mul
  | div
  | eq
  | lt
  | ne
  | le
  | gt
  | ge
  deriving Repr, DecidableEq

inductive Val where
  | intVal : Int → Val
  | ptrVal : Nat → Val
  | boolVal : Bool → Val
  deriving Repr, DecidableEq

abbrev MirEnv := List (Local × Val)
abbrev MirHeap := List Val

def lookup (env : MirEnv) (x : Local) : Option Val :=
  match env with
  | [] => none
  | (y, v) :: rest => if x = y then some v else lookup rest x

def update (env : MirEnv) (x : Local) (v : Val) : MirEnv :=
  (x, v) :: env

def evalOp (op : Op) (v1 v2 : Int) : Option Int :=
  match op with
  | Op.add => some (v1 + v2)
  | Op.sub => some (v1 - v2)
  | Op.mul => some (v1 * v2)
  | Op.div => if v2 = 0 then none else some (v1 / v2)
  | Op.eq  => some (if v1 = v2 then 1 else 0)
  | Op.lt  => some (if v1 < v2 then 1 else 0)
  | Op.ne  => some (if v1 ≠ v2 then 1 else 0)
  | Op.le  => some (if v1 ≤ v2 then 1 else 0)
  | Op.gt  => some (if v1 > v2 then 1 else 0)
  | Op.ge  => some (if v1 ≥ v2 then 1 else 0)

inductive Rvalue where
  | Use : Local → Rvalue
  | Constant : Int → Rvalue
  | BinOp : Op → Local → Local → Rvalue
  | Call : String → List Local → Rvalue
  | Alloc : Local → Rvalue
  | Load : Local → Rvalue
  deriving Repr, DecidableEq

inductive Statement where
  | Assign : Local → Rvalue → Statement
  | Nop : Statement
  deriving Repr, DecidableEq

inductive Terminator where
  | Branch : BasicBlockId → Terminator
  | BranchIf : Local → BasicBlockId → BasicBlockId → Terminator
  | Switch : Local → List (Int × BasicBlockId) → BasicBlockId → Terminator
  | Return : Option Local → Terminator
  | Unreachable : Terminator
  | Fork : BasicBlockId → BasicBlockId → BasicBlockId → Terminator
  deriving Repr, DecidableEq

structure MirBasicBlock where
  id : BasicBlockId
  stmts : List Statement
  term : Terminator
  deriving Repr, DecidableEq

structure MirFunction where
  name : String
  params : List Local
  blocks : List MirBasicBlock
  entry : BasicBlockId
  deriving Repr, DecidableEq

structure MirProgram where
  functions : List MirFunction
  deriving Repr, DecidableEq

structure MirState where
  pc : BasicBlockId
  stmtIdx : Nat
  env : MirEnv
  heap : MirHeap
  deriving Repr, DecidableEq

inductive StmtStep : MirEnv → Statement → MirEnv → Prop where
  | assign_rvalue (env : MirEnv) (x : Local) (v : Val) :
      StmtStep env (Statement.Assign x (Rvalue.Constant 0)) (update env x v)
  | assign_use (env : MirEnv) (x y : Local) (v : Val) :
      lookup env y = some v →
      StmtStep env (Statement.Assign x (Rvalue.Use y)) (update env x v)
  | assign_binop (env : MirEnv) (x y z : Local) (op : Op) (n1 n2 n3 : Int) :
      lookup env y = some (Val.intVal n1) →
      lookup env z = some (Val.intVal n2) →
      evalOp op n1 n2 = some n3 →
      StmtStep env (Statement.Assign x (Rvalue.BinOp op y z)) (update env x (Val.intVal n3))
  | assign_call (env : MirEnv) (x : Local) (f : String) (args : List Local) (retVal : Val) :
      StmtStep env (Statement.Assign x (Rvalue.Call f args)) (update env x retVal)
  | nop (env : MirEnv) :
      StmtStep env Statement.Nop env

inductive Step (fn : MirFunction) : MirState → MirState → Prop where
  | branch (s : MirState) (b : MirBasicBlock) (target : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Branch target →
      Step fn s { s with pc := target, stmtIdx := 0 }
  | branchIf_true (s : MirState) (b : MirBasicBlock) (cond : Local) (then_t else_t : BasicBlockId) (n : Int) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.BranchIf cond then_t else_t →
      lookup s.env cond = some (Val.intVal n) →
      n ≠ 0 →
      Step fn s { s with pc := then_t, stmtIdx := 0 }
  | branchIf_false (s : MirState) (b : MirBasicBlock) (cond : Local) (then_t else_t : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.BranchIf cond then_t else_t →
      lookup s.env cond = some (Val.intVal 0) →
      Step fn s { s with pc := else_t, stmtIdx := 0 }
  | switch (s : MirState) (b : MirBasicBlock) (var : Local) (targets : List (Int × BasicBlockId)) (default_t : BasicBlockId) (target : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Switch var targets default_t →
      Step fn s { s with pc := target, stmtIdx := 0 }
  | ret (s : MirState) (b : MirBasicBlock) (optVal : Option Local) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Return optVal →
      Step fn s s
  | unreachable (s : MirState) (b : MirBasicBlock) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Unreachable →
      Step fn s s
  | fork (s : MirState) (b : MirBasicBlock) (left right join : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      b.term = Terminator.Fork left right join →
      Step fn s { s with pc := join, stmtIdx := 0 }

inductive Evaluates : MirFunction → MirEnv → Val → Prop where
  | base (fn : MirFunction) (env : MirEnv) (retVar : Local) (v : Val) :
      lookup env retVar = some v →
      Evaluates fn env v

def SemanticEquivalent (f1 f2 : MirFunction) : Prop :=
  ∀ args result, Evaluates f1 args result ↔ Evaluates f2 args result

theorem semantic_equiv_refl (f : MirFunction) : SemanticEquivalent f f := by
  intro args res
  exact Iff.rfl

theorem semantic_equiv_symm {f1 f2 : MirFunction} (h : SemanticEquivalent f1 f2) :
    SemanticEquivalent f2 f1 := by
  intro args res
  exact (h args res).symm

theorem semantic_equiv_trans {f1 f2 f3 : MirFunction}
    (h1 : SemanticEquivalent f1 f2) (h2 : SemanticEquivalent f2 f3) :
    SemanticEquivalent f1 f3 := by
  intro args res
  exact (h1 args res).trans (h2 args res)

end Supercompiler
