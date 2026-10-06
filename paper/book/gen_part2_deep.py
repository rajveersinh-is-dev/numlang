#!/usr/bin/env python3
"""
gen_part2_deep.py: Generates deeply expanded text for Part II (Chapters 7 to 14).
"""

import os
from build_350_pages import write_chapter

def get_ch07():
    return r"""\chapter{Symbolic State and Process Trees}
\label{chap:symbolic_state}

\section{The Symbolic State Representation}
\label{sec:symbolic_state:struct}

At the core of the NumLang metacomputation engine lies the \texttt{SymbolicState} struct (\texttt{src/mir/supercompiler/state.rs}). Unlike traditional symbolic execution tools which maintain heavyweight first-order logical constraints for solver-based queries, NumLang models symbolic execution states as lightweight, canonicalized, SSA-aligned algebraic records:

\begin{lstlisting}[language=Rust, caption={The SymbolicState structure in NumLang (\texttt{src/mir/supercompiler/state.rs}).}]
pub struct SymbolicState {
    pub env: HashMap<Place, SymTermId>,
    pub heap: HashMap<SymTermId, SymTermId>,
    pub path_conditions: Vec<PathCondition>,
    pub intervals: HashMap<Place, Interval>,
    pub current_block: BasicBlockId,
    pub call_depth: usize,
    pub loop_depth: usize,
}
\end{lstlisting}

\subsection{Path Conditions and Refinements}
Each \texttt{PathCondition} represents a boolean symbolic fact acquired when driving through conditional branch terminators (\texttt{Terminator::BranchIf} or \texttt{Terminator::Switch}). Path conditions are maintained in conjunctive normal form:
\begin{equation}
\Phi = \bigwedge_{i=1}^m \psi_i, \quad \text{where } \psi_i \in \{\mathtt{BinOp}(\mathtt{Cmp}, t_1, t_2), \mathtt{Discriminant}(v) == k\}
\end{equation}
Path conditions immediately update the \texttt{intervals} domain (Chapter~\ref{chap:refinements}), enabling instantaneous branch pruning without requiring external SMT solver queries during driving.

\section{The SymTerm Intermediate Term Language}
\label{sec:symbolic_state:term_lang}

Symbolic values are modeled by the inductively defined \texttt{SymTerm} language (\texttt{src/mir/supercompiler/term.rs}). A symbolic term represents either a known static scalar, a symbolic input variable, an algebraic combination of terms, an abstract heap reference, or a suspended codata thunk:

\begin{lstlisting}[language=Rust, caption={The SymTerm algebraic datatype in NumLang.}]
pub enum SymTerm {
    ConstInt(i64, Type),
    ConstFloat(u64, Type),
    ConstBool(bool),
    ConstStr(String),
    Var(Place, Type),
    Binary(BinaryOp, SymTermId, SymTermId, Type),
    Unary(UnaryOp, SymTermId, Type),
    Constructor(String, usize, Vec<SymTermId>, Type),
    Select(SymTermId, SymTermId, SymTermId, Type),
    Phi(Vec<(BasicBlockId, SymTermId)>, Type),
    Call(String, Vec<SymTermId>, Type),
    Ref(SymTermId, Type),
    Deref(SymTermId, Type),
    Discriminant(SymTermId, Type),
    ClosureVal(String, Vec<SymTermId>, Type),
    Thunk(String, Vec<SymTermId>, Type),
}
\end{lstlisting}

\subsection{Small-Step Term Reduction Semantics}
The small-step operational semantics of $\symterm$ evaluation under concrete valuation $\sigma : \mathcal{V} \to \mathcal{C}$ are given by the deterministic reduction relation $t \downarrow v$:
\begin{align}
\text{\bf (E-Const)} \quad & \mathtt{ConstInt}(c, \tau) \downarrow c \\[0.8em]
\text{\bf (E-Var)} \quad & \frac{\sigma(x) = v}{\mathtt{Var}(x, \tau) \downarrow v} \\[0.8em]
\text{\bf (E-BinOp)} \quad & \frac{t_1 \downarrow v_1 \quad t_2 \downarrow v_2 \quad v = v_1 \;\text{op}\; v_2}{\mathtt{Binary}(\text{op}, t_1, t_2, \tau) \downarrow v} \\[0.8em]
\text{\bf (E-Select-T)} \quad & \frac{t_{\text{cond}} \downarrow \mathtt{true} \quad t_{\text{then}} \downarrow v}{\mathtt{Select}(t_{\text{cond}}, t_{\text{then}}, t_{\text{else}}, \tau) \downarrow v} \\[0.8em]
\text{\bf (E-Select-F)} \quad & \frac{t_{\text{cond}} \downarrow \mathtt{false} \quad t_{\text{else}} \downarrow v}{\mathtt{Select}(t_{\text{cond}}, t_{\text{then}}, t_{\text{else}}, \tau) \downarrow v}
\end{align}

\section{Hash-Consing via the TermInterner}
\label{sec:symbolic_state:interner}

To ensure that symbolic equivalence checks run in $\mathcal{O}(1)$ time throughout process tree exploration, NumLang implements global \emph{hash-consing} via the \texttt{TermInterner} (\texttt{src/mir/supercompiler/term.rs}). Every unique symbolic term is allocated exactly once and assigned a dense, integer-typed \texttt{SymTermId}.

\subsection{Fast Hashing and Invariants}
Hashing uses a specialized 64-bit rotating hash function (\texttt{fx\_hash\_step}) parameterized by the golden ratio multiplier:
\begin{equation}
h_{k+1} = (h_k \lll 5) \oplus (v \cdot \mathtt{0x517cc1b727220a95})
\end{equation}
The \texttt{TermInterner} maintains the following structural invariants:
\begin{enumerate}
    \item \textbf{Unique Representation}: For any two terms $t_1, t_2$, $\text{lookup}(t_1) = \text{lookup}(t_2) \iff \text{structurally\_identical}(t_1, t_2)$.
    \item \textbf{Monotonic Sizing}: The size of an interned term $t = f(c_1, \dots, c_k)$ is strictly computed as $1 + \sum \text{size}(c_i)$, cached in an array indexable by \texttt{SymTermId}.
    \item \textbf{Subterm Depth}: The depth is cached as $1 + \max \text{depth}(c_i)$, enabling instant whistle size filtering without tree traversal.
\end{enumerate}

\section{The 30 Algebraic Reduction Identities (Phase 59)}
\label{sec:symbolic_state:identities}

During term interning (\texttt{intern\_binary} and \texttt{intern\_unary}), NumLang executes canonical algebraic identity reductions. Rather than generating bloated compound terms that must later be simplified by peephole passes, the interner algebraically normalizes expressions on the fly.

Table~\ref{tab:algebraic_identities} enumerates the 30 foundational identities implemented in Phase 59.

\begin{table}[h!]
\centering
\small
\begin{tabular}{clll}
\toprule
\textbf{\#} & \textbf{Algebraic Form} & \textbf{Normalized Output} & \textbf{Mathematical Rationale} \\
\midrule
1 & $x + 0$ & $x$ & Additive identity \\
2 & $0 + x$ & $x$ & Commutative additive identity \\
3 & $x - 0$ & $x$ & Subtractive right identity \\
4 & $x - x$ & $0$ & Additive inverse nilpotence \\
5 & $0 - x$ & $-x$ & Negation introduction \\
6 & $x \times 1$ & $x$ & Multiplicative identity \\
7 & $1 \times x$ & $x$ & Commutative multiplicative identity \\
8 & $x \times 0$ & $0$ & Multiplicative annihilator \\
9 & $0 \times x$ & $0$ & Commutative multiplicative annihilator \\
10 & $x \times (-1)$ & $-x$ & Multiplicative negation \\
11 & $(-1) \times x$ & $-x$ & Commutative multiplicative negation \\
12 & $x / 1$ & $x$ & Divisive identity \\
13 & $x / (-1)$ & $-x$ & Divisive negation \\
14 & $x / x \quad (x \ne 0)$ & $1$ & Multiplicative inverse identity \\
15 & $0 / x \quad (x \ne 0)$ & $0$ & Zero numerator absorption \\
16 & $x \pmod 1$ & $0$ & Unit modulus nullity \\
17 & $x \pmod x \quad (x \ne 0)$ & $0$ & Self-modulus absorption \\
18 & $0 \pmod x \quad (x \ne 0)$ & $0$ & Zero modulus nullity \\
19 & $x \ll 0$ & $x$ & Zero bit-shift nullity \\
20 & $0 \ll k$ & $0$ & Zero shifted absorption \\
21 & $x \gg 0$ & $x$ & Zero arithmetic shift nullity \\
22 & $0 \gg k$ & $0$ & Zero shifted absorption \\
23 & $x \mathbin{\&} x$ & $x$ & Bitwise AND idempotence \\
24 & $x \mathbin{\&} 0$ & $0$ & Bitwise AND annihilator \\
25 & $x \mathbin{\&} (-1)$ & $x$ & Bitwise AND full-mask identity \\
26 & $x \mathbin{|} x$ & $x$ & Bitwise OR idempotence \\
27 & $x \mathbin{|} 0$ & $x$ & Bitwise OR identity \\
28 & $x \mathbin{|} (-1)$ & $-1$ & Bitwise OR full-mask annihilator \\
29 & $x \oplus x$ & $0$ & Bitwise XOR self-inversion \\
30 & $x \oplus 0$ & $x$ & Bitwise XOR identity \\
\bottomrule
\end{tabular}
\caption{The 30 fundamental algebraic identities enforced during term interning.}
\label{tab:algebraic_identities}
\end{table}

\subsection{Formal Proofs of Selected Identities in Modular Arithmetic}
Because NumLang operates over fixed-width 64-bit two's-complement integers, all operations are evaluated in the commutative ring $\mathbb{Z} / 2^{64}\mathbb{Z}$.

\begin{theorem}[Bitwise XOR Self-Inversion]
For any bitvector $x \in \{0, 1\}^{64}$, $x \oplus x = 0$.
\end{theorem}
\begin{proof}
By definition of bitwise XOR, $(x \oplus y)_k = x_k \oplus y_k$. For $x = y$, $x_k \oplus x_k = 0$ for all $k \in \{0, \dots, 63\}$. Thus, the resulting bitvector is identically zero.
\end{proof}

\begin{theorem}[Reassociation Invariant]
In $\mathbb{Z} / 2^{64}\mathbb{Z}$, $(x + c_1) + c_2 = x + (c_1 + c_2)$.
\end{theorem}
\begin{proof}
Addition modulo $2^{64}$ is associative. The compiler evaluates $c_1 + c_2 \pmod{2^{64}}$ at compile time via wrapping arithmetic, replacing two runtime addition operations with a single addition against the pre-folded constant.
\end{proof}

\section{Process Trees: Nodes, Edges, and Topology}
\label{sec:symbolic_state:trees}

The execution of the supercompiler constructs an explicit directed hypergraph termed the \emph{Process Tree} ($\proctree$). Nodes in the process tree are categorized into four canonical variants:

\begin{enumerate}
    \item \textbf{Leaf Nodes}: Terminal evaluation states representing either an explicit return value (\texttt{Terminator::Return}), an unreachable state resulting from interval dead-branch pruning, or a runtime divergence.
    \item \textbf{Branch Nodes}: Control-flow decision points where driving bifurcated into multiple child configurations (corresponding to conditional branches or pattern matches).
    \item \textbf{Knot Nodes}: Generalization loop headers where recursive execution has folded into a fixed point configuration. A knot node defines the entry signature for a synthesized recursive residual function.
    \item \textbf{Fold Nodes}: Leaf-level back-edges referencing an ancestor knot node. A fold node supplies the concrete arguments for the recursive invocation.
\end{enumerate}

\section{Knot Allocation and Generalization Fallback}
\label{sec:symbolic_state:knots}

When the whistle detects that a newly reached configuration $C_{\text{curr}}$ homeomorphically embeds an ancestor configuration $C_{\text{anc}}$, the driving engine halts linear evaluation:

\begin{enumerate}
    \item If $C_{\text{curr}}$ is an exact $\alpha$-rename of $C_{\text{anc}}$, a fold node is materialized pointing to $C_{\text{anc}}$, closing the recursive loop.
    \item If $C_{\text{curr}}$ structurally contains $C_{\text{anc}}$ with varying accumulator values, the supercompiler invokes \emph{Most Specific Generalization} (MSG, Chapter~\ref{chap:generalization}).
    \item The MSG algorithm computes the common generalization $C_{\text{gen}} = \text{MSG}(C_{\text{anc}}, C_{\text{curr}})$. The original subtree rooted at $C_{\text{anc}}$ is pruned and replaced by $C_{\text{gen}}$, which is promoted to a Knot node. Driving resumes from $C_{\text{gen}}$.
\end{enumerate}

\section{Residualization: Synthesizing SSA MIR from Trees}
\label{sec:symbolic_state:residualize}

Once all branches of the process tree have terminated at either Leaves or Folds, the supercompiler executes \emph{residualization} (\texttt{src/mir/supercompiler/residualize.rs}):

\begin{enumerate}
    \item \textbf{Function Synthesis}: Each Knot node is allocated a fresh residual function signature $\mathtt{res\_fn\_}k(\vec{p})$.
    \item \textbf{Block Emission}: Tree edges are linearized into SSA basic blocks. Branch nodes emit \texttt{Terminator::BranchIf} or \texttt{Terminator::Switch}.
    \item \textbf{Fold Lowering}: Fold nodes emit direct function calls $\mathtt{Call}(\mathtt{res\_fn\_}k, \vec{args})$, completing the synthesized recurrence.
    \item \textbf{SSA Verification}: The residualized function undergoes standard dominance and SSA validation prior to native backend lowering.
\end{enumerate}
"""

def build():
    write_chapter("07_symbolic_state.tex", get_ch07())

if __name__ == "__main__":
    build()
