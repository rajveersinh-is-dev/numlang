namespace Supercompiler

abbrev Local := String
abbrev BasicBlockId := Nat

inductive Op where
  | add
  | sub
  | mul
  | div
  | mod
  | bit_and
  | bit_or
  | bit_xor
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

def intToUInt64 (v : Int) : UInt64 :=
  let mod2_64 : Nat := 18446744073709551616
  if v >= 0 then
    UInt64.ofNat (v.toNat % mod2_64)
  else
    let rem := v.natAbs % mod2_64
    if rem = 0 then 0
    else UInt64.ofNat (mod2_64 - rem)

def uInt64ToInt (u : UInt64) : Int :=
  let n := u.toNat
  let pow2_63 : Nat := 9223372036854775808
  let pow2_64 : Nat := 18446744073709551616
  if n >= pow2_63 then
    - (Int.ofNat (pow2_64 - n))
  else
    Int.ofNat n

def evalOp (op : Op) (v1 v2 : Int) : Option Int :=
  match op with
  | Op.add => some (v1 + v2)
  | Op.sub => some (v1 - v2)
  | Op.mul => some (v1 * v2)
  | Op.div =>
    if v2 = 0 then none
    else
      let d := Int.ofNat (v1.natAbs / v2.natAbs)
      some (if (v1 < 0) == (v2 < 0) then d else -d)
  | Op.mod =>
    if v2 = 0 then none
    else
      let rem := Int.ofNat (v1.natAbs % v2.natAbs)
      some (if v1 < 0 then -rem else rem)
  | Op.bit_and => some (uInt64ToInt (intToUInt64 v1 &&& intToUInt64 v2))
  | Op.bit_or  => some (uInt64ToInt (intToUInt64 v1 ||| intToUInt64 v2))
  | Op.bit_xor => some (uInt64ToInt (intToUInt64 v1 ^^^ intToUInt64 v2))
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
  | assign_const (env : MirEnv) (x : Local) (n : Int) :
      StmtStep env (Statement.Assign x (Rvalue.Constant n)) (update env x (Val.intVal n))
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

def getStmt (stmts : List Statement) (idx : Nat) : Option Statement :=
  match stmts, idx with
  | [], _ => none
  | x :: _, 0 => some x
  | _ :: xs, n + 1 => getStmt xs n

inductive Step (fn : MirFunction) : MirState → MirState → Prop where
  | stmt (s : MirState) (b : MirBasicBlock) (st : Statement) (env' : MirEnv) :
      b ∈ fn.blocks →
      b.id = s.pc →
      getStmt b.stmts s.stmtIdx = some st →
      StmtStep s.env st env' →
      Step fn s { s with stmtIdx := s.stmtIdx + 1, env := env' }
  | branch (s : MirState) (b : MirBasicBlock) (target : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.Branch target →
      Step fn s { s with pc := target, stmtIdx := 0 }
  | branchIf_true (s : MirState) (b : MirBasicBlock) (cond : Local) (then_t else_t : BasicBlockId) (n : Int) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.BranchIf cond then_t else_t →
      lookup s.env cond = some (Val.intVal n) →
      n ≠ 0 →
      Step fn s { s with pc := then_t, stmtIdx := 0 }
  | branchIf_false (s : MirState) (b : MirBasicBlock) (cond : Local) (then_t else_t : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.BranchIf cond then_t else_t →
      lookup s.env cond = some (Val.intVal 0) →
      Step fn s { s with pc := else_t, stmtIdx := 0 }
  | switch (s : MirState) (b : MirBasicBlock) (var : Local) (targets : List (Int × BasicBlockId)) (default_t : BasicBlockId) (target : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.Switch var targets default_t →
      Step fn s { s with pc := target, stmtIdx := 0 }
  | fork (s : MirState) (b : MirBasicBlock) (left right join : BasicBlockId) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.Fork left right join →
      Step fn s { s with pc := join, stmtIdx := 0 }

inductive TerminatesWith (fn : MirFunction) (s : MirState) (res : Val) : Prop where
  | ret_some (b : MirBasicBlock) (retVar : Local) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.Return (some retVar) →
      lookup s.env retVar = some res →
      TerminatesWith fn s res
  | ret_none (b : MirBasicBlock) :
      b ∈ fn.blocks →
      b.id = s.pc →
      s.stmtIdx = b.stmts.length →
      b.term = Terminator.Return none →
      res = Val.intVal 0 →
      TerminatesWith fn s res

inductive StepStar (fn : MirFunction) : MirState → MirState → Prop where
  | refl (s : MirState) : StepStar fn s s
  | step (s1 s2 s3 : MirState) : Step fn s1 s2 → StepStar fn s2 s3 → StepStar fn s1 s3

theorem stepstar_trans {fn : MirFunction} {s1 s2 s3 : MirState}
    (h1 : StepStar fn s1 s2) (h2 : StepStar fn s2 s3) :
    StepStar fn s1 s3 := by
  induction h1 with
  | refl _ => exact h2
  | step s_a s_b s_c hstep _ ih =>
    exact StepStar.step s_a s_b s3 hstep (ih h2)

theorem stepstar_single {fn : MirFunction} {s1 s2 : MirState}
    (h : Step fn s1 s2) : StepStar fn s1 s2 :=
  StepStar.step s1 s2 s2 h (StepStar.refl s2)

def initState (fn : MirFunction) (args : MirEnv) : MirState :=
  { pc := fn.entry, stmtIdx := 0, env := args, heap := [] }

def Evaluates (fn : MirFunction) (args : MirEnv) (res : Val) : Prop :=
  ∃ s_final, StepStar fn (initState fn args) s_final ∧ TerminatesWith fn s_final res

def SemanticEquivalent (f1 f2 : MirFunction) : Prop :=
  ∀ args res, Evaluates f1 args res ↔ Evaluates f2 args res

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

theorem const_fold_stmt_equiv (env : MirEnv) (x y z : Local) (op : Op) (n1 n2 n3 : Int)
    (hy : lookup env y = some (Val.intVal n1))
    (hz : lookup env z = some (Val.intVal n2))
    (hop : evalOp op n1 n2 = some n3) :
    StmtStep env (Statement.Assign x (Rvalue.BinOp op y z)) (update env x (Val.intVal n3)) ∧
    StmtStep env (Statement.Assign x (Rvalue.Constant n3)) (update env x (Val.intVal n3)) := by
  constructor
  · exact StmtStep.assign_binop env x y z op n1 n2 n3 hy hz hop
  · exact StmtStep.assign_const env x n3

end Supercompiler
