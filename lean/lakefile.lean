import Lake
open Lake DSL

package «supercompiler-proofs» where
  leanOptions := #[
    ⟨`autoImplicit, false⟩,
    ⟨`relaxedAutoImplicit, false⟩
  ]

@[default_target]
lean_lib «Supercompiler» where

lean_exe «lean_eval» where
  root := `LeanEval
