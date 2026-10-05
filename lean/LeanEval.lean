import Lean
import Supercompiler.Semantics

open Lean
open Supercompiler

set_option linter.unusedVariables false

inductive JOp where
  | add | sub | mul | div | mod | bit_and | bit_or | bit_xor | eq | lt | ne | le | gt | ge
  deriving FromJson, ToJson

def JOp.toOp : JOp → Op
  | JOp.add => Op.add
  | JOp.sub => Op.sub
  | JOp.mul => Op.mul
  | JOp.div => Op.div
  | JOp.mod => Op.mod
  | JOp.bit_and => Op.bit_and
  | JOp.bit_or  => Op.bit_or
  | JOp.bit_xor => Op.bit_xor
  | JOp.eq  => Op.eq
  | JOp.lt  => Op.lt
  | JOp.ne  => Op.ne
  | JOp.le  => Op.le
  | JOp.gt  => Op.gt
  | JOp.ge  => Op.ge

inductive JRvalue where
  | use (var : String)
  | const (val : Int)
  | binop (op : JOp) (left right : String)
  | call (f : String) (args : List String)
  | alloc (var : String)
  | load (var : String)
  deriving FromJson, ToJson

def JRvalue.toRvalue : JRvalue → Rvalue
  | JRvalue.use v => Rvalue.Use v
  | JRvalue.const n => Rvalue.Constant n
  | JRvalue.binop op l r => Rvalue.BinOp op.toOp l r
  | JRvalue.call f args => Rvalue.Call f args
  | JRvalue.alloc v => Rvalue.Alloc v
  | JRvalue.load v => Rvalue.Load v

inductive JStatement where
  | assign (var : String) (rv : JRvalue)
  | nop
  deriving FromJson, ToJson

def JStatement.toStatement : JStatement → Statement
  | JStatement.assign v rv => Statement.Assign v rv.toRvalue
  | JStatement.nop => Statement.Nop

inductive JTerminator where
  | branch (target : Nat)
  | branch_if (cond : String) (then_t else_t : Nat)
  | switch (var : String) (targets : List (Int × Nat)) (default_t : Nat)
  | ret (var : Option String)
  | unreachable
  | fork (left right join : Nat)
  deriving FromJson, ToJson

def JTerminator.toTerminator : JTerminator → Terminator
  | JTerminator.branch t => Terminator.Branch t
  | JTerminator.branch_if c t e => Terminator.BranchIf c t e
  | JTerminator.switch v ts d => Terminator.Switch v ts d
  | JTerminator.ret v => Terminator.Return v
  | JTerminator.unreachable => Terminator.Unreachable
  | JTerminator.fork l r j => Terminator.Fork l r j

structure JBasicBlock where
  id : Nat
  stmts : List JStatement
  term : JTerminator
  deriving FromJson, ToJson

def JBasicBlock.toBasicBlock (b : JBasicBlock) : MirBasicBlock :=
  { id := b.id, stmts := b.stmts.map JStatement.toStatement, term := b.term.toTerminator }

structure JFunction where
  name : String
  params : List String
  entry : Nat
  blocks : List JBasicBlock
  deriving FromJson, ToJson

def JFunction.toFunction (f : JFunction) : MirFunction :=
  { name := f.name, params := f.params, entry := f.entry, blocks := f.blocks.map JBasicBlock.toBasicBlock }

structure JProgram where
  functions : List JFunction
  deriving FromJson, ToJson

def JProgram.toProgram (p : JProgram) : MirProgram :=
  { functions := p.functions.map JFunction.toFunction }

def findBlock (blocks : List MirBasicBlock) (id : BasicBlockId) : Option MirBasicBlock :=
  match blocks with
  | [] => none
  | b :: bs => if b.id = id then some b else findBlock bs id

def findFunc (funcs : List MirFunction) (name : String) : Option MirFunction :=
  match funcs with
  | [] => none
  | f :: fs => if f.name = name then some f else findFunc fs name

def getHeapVal (heap : MirHeap) (idx : Nat) : Option Val :=
  match heap, idx with
  | [], _ => none
  | v :: _, 0 => some v
  | _ :: vs, n + 1 => getHeapVal vs n

def bindArgs (params : List Local) (args : List Local) (env : MirEnv) : Option MirEnv :=
  match params, args with
  | [], [] => some []
  | p :: ps, a :: as =>
    match lookup env a with
    | some v =>
      match bindArgs ps as env with
      | some rest => some ((p, v) :: rest)
      | none => none
    | none => none
  | _, _ => none

partial def evalFn (prog : MirProgram) (fn : MirFunction) (args : MirEnv) (heap : MirHeap) (fuel : Nat) :
    Option (Val × MirHeap × Nat) :=
  let s0 : MirState := { pc := fn.entry, stmtIdx := 0, env := args, heap := heap }
  stepLoop prog fn s0 fuel

where
  stepLoop (prog : MirProgram) (fn : MirFunction) (s : MirState) (fuel : Nat) :
      Option (Val × MirHeap × Nat) :=
    match fuel with
    | 0 => none
    | fuel' + 1 =>
      match findBlock fn.blocks s.pc with
      | none => none
      | some b =>
        if s.stmtIdx < b.stmts.length then
          match getStmt b.stmts s.stmtIdx with
          | none => none
          | some st =>
            match st with
            | Statement.Assign x (Rvalue.Constant n) =>
              let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x (Val.intVal n) }
              stepLoop prog fn s' fuel'
            | Statement.Assign x (Rvalue.Use y) =>
              match lookup s.env y with
              | some v =>
                let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x v }
                stepLoop prog fn s' fuel'
              | none => none
            | Statement.Assign x (Rvalue.BinOp op y z) =>
              match lookup s.env y, lookup s.env z with
              | some (Val.intVal n1), some (Val.intVal n2) =>
                match evalOp op n1 n2 with
                | some n3 =>
                  let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x (Val.intVal n3) }
                  stepLoop prog fn s' fuel'
                | none => none
              | _, _ => none
            | Statement.Assign x (Rvalue.Call callee args) =>
              match findFunc prog.functions callee with
              | none => none
              | some calleeFn =>
                match bindArgs calleeFn.params args s.env with
                | none => none
                | some calleeEnv =>
                  match evalFn prog calleeFn calleeEnv s.heap fuel' with
                  | some (retVal, heap', remFuel) =>
                    let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x retVal, heap := heap' }
                    stepLoop prog fn s' remFuel
                  | none => none
            | Statement.Assign x (Rvalue.Alloc y) =>
              match lookup s.env y with
              | some v =>
                let ptr := s.heap.length
                let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x (Val.ptrVal ptr), heap := s.heap ++ [v] }
                stepLoop prog fn s' fuel'
              | none => none
            | Statement.Assign x (Rvalue.Load y) =>
              match lookup s.env y with
              | some (Val.ptrVal p) =>
                match getHeapVal s.heap p with
                | some v =>
                  let s' := { s with stmtIdx := s.stmtIdx + 1, env := update s.env x v }
                  stepLoop prog fn s' fuel'
                | none => none
              | _ => none
            | Statement.Nop =>
              let s' := { s with stmtIdx := s.stmtIdx + 1 }
              stepLoop prog fn s' fuel'
        else
          match b.term with
          | Terminator.Branch tgt =>
            let s' := { s with pc := tgt, stmtIdx := 0 }
            stepLoop prog fn s' fuel'
          | Terminator.BranchIf cond then_t else_t =>
            match lookup s.env cond with
            | some (Val.intVal n) =>
              let tgt := if n ≠ 0 then then_t else else_t
              let s' := { s with pc := tgt, stmtIdx := 0 }
              stepLoop prog fn s' fuel'
            | _ => none
          | Terminator.Switch var targets def_t =>
            match lookup s.env var with
            | some (Val.intVal n) =>
              let rec findTgt (ts : List (Int × BasicBlockId)) : BasicBlockId :=
                match ts with
                | [] => def_t
                | (k, t) :: rest => if k = n then t else findTgt rest
              let s' := { s with pc := findTgt targets, stmtIdx := 0 }
              stepLoop prog fn s' fuel'
            | _ => none
          | Terminator.Fork _ _ join =>
            let s' := { s with pc := join, stmtIdx := 0 }
            stepLoop prog fn s' fuel'
          | Terminator.Return (some retVar) =>
            match lookup s.env retVar with
            | some v => some (v, s.heap, fuel')
            | none => none
          | Terminator.Return none =>
            some (Val.intVal 0, s.heap, fuel')
          | Terminator.Unreachable =>
            none

def main : IO Unit := do
  let stdin ← IO.getStdin
  let input ← stdin.readToEnd
  match Json.parse input >>= fromJson? (α := JProgram) with
  | Except.error e =>
    IO.println ("{\"status\":\"error\",\"message\":" ++ (Json.str e).compress ++ "}")
  | Except.ok jp =>
    let prog := jp.toProgram
    match findFunc prog.functions "main" with
    | none =>
      IO.println "{\"status\":\"error\",\"message\":\"Function 'main' not found\"}"
    | some mainFn =>
      match evalFn prog mainFn [] [] 10000000 with
      | some (Val.intVal n, _, _) =>
        IO.println ("{\"status\":\"ok\",\"value\":" ++ toString n ++ "}")
      | some (Val.boolVal b, _, _) =>
        let n := if b then 1 else 0
        IO.println ("{\"status\":\"ok\",\"value\":" ++ toString n ++ "}")
      | some (Val.ptrVal p, _, _) =>
        IO.println ("{\"status\":\"ok\",\"value\":" ++ toString p ++ "}")
      | none =>
        IO.println "{\"status\":\"diverged\"}"
