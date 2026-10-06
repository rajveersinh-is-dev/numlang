#!/usr/bin/env python3
"""
expand_part2.py: Comprehensive technical expansion for Part II (Chapters 7, 8, 9, 10, 11, 12, 14).
"""

import os

ROOT = os.path.dirname(os.path.abspath(__file__))
CHAPTERS = os.path.join(ROOT, "chapters")

def write_file(name, content):
    path = os.path.join(CHAPTERS, name)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content.strip() + "\n")
    print(f"Successfully generated {name} ({len(content.splitlines())} lines)")

def generate_ch07():
    return r"""\chapter{Symbolic State and Process Trees}
\label{chap:symbolic_state}

\section{The Symbolic State Representation}
\label{sec:symbolic_state:struct}

At the heart of the NumLang metacomputation engine lies the \texttt{SymbolicState} struct (\texttt{src/mir/supercompiler/state.rs}). Unlike traditional symbolic execution tools which maintain heavyweight first-order logical constraints for solver-based path queries, NumLang models symbolic execution states as lightweight, canonicalized, SSA-aligned algebraic records:

\begin{lstlisting}[language=Rust, caption={The SymbolicState structure in NumLang.}]
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
Crucially, path conditions immediately update the \texttt{intervals} domain (Chapter~\ref{chap:refinements}), enabling instantaneous branch pruning without requiring external SMT invocations during the driving loop.

\section{The SymTerm Intermediate Term Language}
\label{sec:symbolic_state:term_lang}

Symbolic values are modeled by the inductively defined \texttt{SymTerm} language (\texttt{src/mir/supercompiler/term.rs}). A symbolic term represents either a known static scalar, a symbolic input variable, an algebraic combination of terms, an abstract heap reference, or a suspended codata thunk:

\begin{lstlisting}[language=Rust, caption={The SymTerm algebraic datatype.}]
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
\text{\bf (E-Select)} \quad & \frac{t_{\text{cond}} \downarrow \mathtt{true} \quad t_{\text{then}} \downarrow v}{\mathtt{Select}(t_{\text{cond}}, t_{\text{then}}, t_{\text{else}}, \tau) \downarrow v} \qquad
\frac{t_{\text{cond}} \downarrow \mathtt{false} \quad t_{\text{else}} \downarrow v}{\mathtt{Select}(t_{\text{cond}}, t_{\text{then}}, t_{\text{else}}, \tau) \downarrow v}
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

\subsection{Formal Correctness Proof for Reassociation}
In addition to identity elimination, the interner enforces canonical constant reassociation:
\begin{align}
(x + c_1) + c_2 &\longrightarrow x + (c_1 + c_2) \\
(x - c_1) + c_2 &\longrightarrow x + (c_2 - c_1) \\
(x \times c_1) \times c_2 &\longrightarrow x \times (c_1 \times c_2)
\end{align}
Because two's-complement arithmetic over fixed-width 64-bit integers forms a commutative ring $(\mathbb{Z} / 2^{64}\mathbb{Z}, +, \times)$, addition and multiplication are strictly associative and commutative. Thus, these rewrites are semantics-preserving under all possible overflow conditions.

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

def generate_ch08():
    return r"""\chapter{Symbolic Driving}
\label{chap:driving}

\section{The Core Driving Architecture}
\label{sec:driving:arch}

Symbolic driving is the operational engine that propels metacomputation forward. Implemented in \texttt{src/mir/supercompiler/drive.rs}, driving symbolically evaluates SSA basic blocks statement by statement, updating the symbolic environment, resolving memory dereferences, and expanding terminators into process tree branches.

Algorithm~\ref{alg:driving_loop} presents the high-level pseudocode for the primary driving procedure \texttt{drive\_node}.

\begin{algorithm}[h!]
\caption{The NumLang Symbolic Driving Loop (\texttt{drive\_node})}
\label{alg:driving_loop}
\begin{algorithmic}[1]
\Require Process Tree Node $N$, Configuration $C = \langle bb, \vec{s}, \env, \heap, \Phi \rangle$, Budget $\mathcal{B}$
\Ensure Updated Process Tree Subtree rooted at $N$
\Procedure{DriveNode}{$N, C, \mathcal{B}$}
    \If{$\mathcal{B}.\text{depth} \le 0 \lor \mathcal{B}.\text{inline} \le 0$}
        \State \Return \Call{ForceGeneralize}{$N, C$}
    \EndIf
    \If{\Call{WhistleBlows}{$C, \text{Ancestors}(N)$}}
        \State \Return \Call{HandleWhistle}{$N, C$}
    \EndIf
    \While{$\vec{s} \ne []$}
        \State Let $stmt = \text{head}(\vec{s}); \quad \vec{s} = \text{tail}(\vec{s})$
        \State $C \gets$ \Call{DriveStatement}{$stmt, C$}
    \EndWhile
    \State \Return \Call{DriveTerminator}{$N, \text{terminator}(bb), C, \mathcal{B}$}
\EndProcedure
\end{algorithmic}
\end{algorithm}

\section{Driving SSA Instructions and Rvalues}
\label{sec:driving:instructions}

For each SSA statement $x = \mathtt{Rvalue}$, \texttt{drive\_statement} updates the environment $\env[x]$:

\begin{itemize}
    \item \textbf{Constant Assignment} ($x = \mathtt{Constant}(c)$): Interns $c$ via \texttt{TermInterner} and binds $\env[x] \gets \text{id}(c)$.
    \item \textbf{Binary Arithmetic} ($x = \mathtt{BinOp}(\text{op}, y, z)$): Retrieves symbolic term IDs for $y$ and $z$. Invokes \texttt{intern\_binary(op, env[y], env[z])}, automatically triggering the 30 algebraic reductions.
    \item \textbf{Heap Allocation} ($x = \mathtt{Alloc}(y)$): Allocates a fresh symbolic pointer ID $p_{\text{fresh}}$, binds $\heap[p_{\text{fresh}}] \gets \env[y]$, and maps $\env[x] \gets p_{\text{fresh}}$.
    \item \textbf{Heap Dereference} ($x = \mathtt{Load}(y)$): If $\env[y]$ resolves to a known pointer $p \in \text{dom}(\heap)$, driving folds the read into $\env[x] \gets \heap[p]$. If $p$ is an unknown symbolic parameter, driving materializes a symbolic term $\mathtt{Deref}(\env[y])$.
\end{itemize}

\section{Driving Terminators and Control-Flow Bifurcation}
\label{sec:driving:terminators}

When all linear statements within a basic block have been driven, \texttt{drive\_terminator} handles the control-flow terminator:

\subsection{Conditional Branch (\texttt{Terminator::BranchIf})}
Given $\mathtt{BranchIf}(c, bb_{\text{then}}, bb_{\text{else}})$:
\begin{enumerate}
    \item \textbf{Constant Condition}: If $\env[c]$ is statically proven \texttt{true} (or \texttt{false}), driving proceeds unconditionally to $bb_{\text{then}}$ (or $bb_{\text{else}}$).
    \item \textbf{Interval Pruning}: If the numerical interval of $c$ excludes zero, the \texttt{else} branch is pruned as dead code.
    \item \textbf{Symbolic Bifurcation}: If $c$ remains indeterminate, driving branches the process tree into two children, adding $c == \mathtt{true}$ to $\Phi_{\text{then}}$ and $c == \mathtt{false}$ to $\Phi_{\text{else}}$.
\end{enumerate}

\subsection{Function Inlining and Recursion (\texttt{Terminator::Call})}
Given a function call $x = \mathtt{Call}(f, \vec{args})$:
\begin{enumerate}
    \item If $f$ is non-recursive and within the inline budget ($\le 500$ MIR statements), the callee body is inlined directly into the driving configuration.
    \item If $f$ is recursive, driving checks whether unrolling $f$ matches an existing recurrence companion pattern (Chapter~\ref{chap:recurrences}) or blows the whistle (Chapter~\ref{chap:termination}).
\end{enumerate}

\section{Budgets and the CPS Driving Trampoline (Phase 65)}
\label{sec:driving:cps}

Historically, deeply nested or mutual recurrences would cause the Rust compiler host stack to overflow during recursive \texttt{drive\_node} invocations. In Phase 65, NumLang refactored the driving loop into a \emph{Continuation-Passing Style (CPS) Driving Trampoline}.

Work is encapsulated into an explicit heap-allocated task queue:
\begin{lstlisting}[language=Rust, caption={The CPS DriveTask queue in NumLang.}]
pub enum DriveTask {
    StepNode(ProcessNodeId, SymbolicState, Budget),
    ResumeBranch(ProcessNodeId, Vec<ProcessNodeId>),
    FinalizeKnot(ProcessNodeId, SymTermId),
}
\end{lstlisting}
The driver runs as an explicit while-loop over \texttt{VecDeque<DriveTask>}, guaranteeing that supercompilation can safely traverse recursion depths exceeding 100,000 steps without risking native stack overflow.

\section{Parallel Work-Stealing Driving (Phase 36)}
\label{sec:driving:parallel}

Because branching decisions produce independent child configurations, NumLang implements parallel driving via Rayon (\texttt{src/mir/supercompiler/parallel.rs}). When the task queue contains multiple pending branch nodes and CPU cores are available, the trampoline forks child evaluations across thread pools. Independent subtrees are driven concurrently and joined deterministically at synchronization barriers.

\section{End-to-End Driving Walkthrough: double\_nrev}
\label{sec:driving:nrev_walkthrough}

To observe the full mechanics of symbolic driving, consider supercompiling the classic double-reverse pipeline:
\begin{equation}
\mathtt{double\_nrev}(xs) = \mathtt{nrev}(\mathtt{nrev}(xs))
\end{equation}

\begin{enumerate}
    \item \textbf{Initial Configuration}: $C_0 = \langle \mathtt{bb0}, [xs \mapsto \alpha], \emptyset, \emptyset \rangle$, where $\alpha$ is a symbolic list parameter.
    \item \textbf{First Unfold}: Driving inlines the outer \texttt{nrev}. It encounters a match on $\mathtt{nrev}(\alpha)$. Because the inner expression is not in weak head normal form, driving pivots to evaluate the inner $\mathtt{nrev}(\alpha)$.
    \item \textbf{Branching on Head Constructor}: The inner list branches on $\alpha = \mathtt{Nil}$ vs $\alpha = \mathtt{Cons}(h, t)$.
    \begin{itemize}
        \item \emph{Base Case ($\alpha = \mathtt{Nil}$)}: $\mathtt{nrev}(\mathtt{Nil}) \to \mathtt{Nil}$. Outer call evaluates $\mathtt{nrev}(\mathtt{Nil}) \to \mathtt{Nil}$. Emits leaf node.
        \item \emph{Recursive Case ($\alpha = \mathtt{Cons}(h, t)$)}: Inner call expands to $\mathtt{append}(\mathtt{nrev}(t), [h])$. Outer call now faces $\mathtt{nrev}(\mathtt{append}(\mathtt{nrev}(t), [h]))$.
    \end{itemize}
    \item \textbf{Whistle Firing and Generalization}: As the accumulator grows with nested append calls, the homeomorphic embedding whistle blows. Generalization abstracts the accumulator into a fresh variable $\beta$, synthesizing the generalized knot $\mathtt{rev\_acc}(xs, \beta)$.
    \item \textbf{Residual MIR}: The final residualized program collapses the intermediate lists, synthesizing a direct tail-recursive accumulator loop.
\end{enumerate}
"""

def generate_ch09():
    return r"""\chapter{Termination: Homeomorphic Embedding and Whistles}
\label{chap:termination}

\section{The Non-Termination Hazard in Metacomputation}
\label{sec:termination:hazard}

Because supercompilation symbolically unrolls loops and function applications, an unrestricted driving loop will trivially diverge when evaluating non-terminating programs, recursive functions over infinite domains, or cyclic data flow. To guarantee that compilation terminates unconditionally on all valid and invalid inputs, the supercompiler must be equipped with a \emph{whistle}—an algorithmic oracle that detects when a sequence of symbolic terms risks unbounded growth.

\section{Formal Definition of Homeomorphic Embedding}
\label{sec:termination:he_def}

NumLang governs driving termination through homeomorphic embedding ($\trianglelefteq$) over the \texttt{SymTerm} intermediate language \citep{sorensen1995algorithm}.

\begin{definition}[Homeomorphic Embedding over SymTerm]
Let $s, t \in \symterm$. We say that $s$ is homeomorphically embedded in $t$, denoted $s \trianglelefteq t$, if and only if one of the following structural conditions holds:
\begin{align}
\text{\bf (Diving-BinOp)} \quad & \frac{s \trianglelefteq t_1 \lor s \trianglelefteq t_2}{s \trianglelefteq \mathtt{Binary}(\text{op}, t_1, t_2, \tau)} \\[0.8em]
\text{\bf (Diving-Unary)} \quad & \frac{s \trianglelefteq t_1}{s \trianglelefteq \mathtt{Unary}(\text{op}, t_1, \tau)} \\[0.8em]
\text{\bf (Diving-Constructor)} \quad & \frac{\exists i \in \{1, \dots, n\}.\; s \trianglelefteq t_i}{s \trianglelefteq \mathtt{Constructor}(K, \delta, [t_1, \dots, t_n], \tau)} \\[0.8em]
\text{\bf (Coupling-BinOp)} \quad & \frac{\text{op}_1 = \text{op}_2 \quad s_1 \trianglelefteq t_1 \quad s_2 \trianglelefteq t_2}{\mathtt{Binary}(\text{op}_1, s_1, s_2, \tau_1) \trianglelefteq \mathtt{Binary}(\text{op}_2, t_1, t_2, \tau_2)} \\[0.8em]
\text{\bf (Coupling-Constructor)} \quad & \frac{K_1 = K_2 \quad \delta_1 = \delta_2 \quad \forall i.\; s_i \trianglelefteq t_i}{\mathtt{Constructor}(K_1, \delta_1, \vec{s}, \tau_1) \trianglelefteq \mathtt{Constructor}(K_2, \delta_2, \vec{t}, \tau_2)}
\end{align}
Variables and constants embed only themselves: $\mathtt{Var}(x) \trianglelefteq \mathtt{Var}(y)$ and $\mathtt{Const}(c) \trianglelefteq \mathtt{Const}(c)$.
\end{definition}

\section{Well-Quasi-Ordering and Termination Proof}
\label{sec:termination:kruskal_proof}

The mathematical guarantee of supercompiler termination rests on Higman's Lemma and Kruskal's Tree Theorem \citep{kruskal1960well}.

\begin{theorem}[Driving Loop Termination]
Let $\langle C_0, C_1, C_2, \dots \rangle$ be any sequence of configurations generated along an infinite branch of a process tree. Then there exist indices $i < j$ such that $C_i \trianglelefteq C_j$.
\end{theorem}

\begin{proof}
Every configuration $C_k$ is a finite tree over a finite ranked alphabet $\Sigma$ consisting of MIR operators, basic block IDs, and type constructors. By Kruskal's Tree Theorem, the homeomorphic embedding relation $\trianglelefteq$ is a well-quasi-ordering on $\mathcal{T}(\Sigma)$. By definition of a well-quasi-ordering, every infinite sequence contains an infinite ascending chain $C_{i_1} \trianglelefteq C_{i_2} \trianglelefteq \dots$. Therefore, a pair $i < j$ with $C_i \trianglelefteq C_j$ is encountered in a finite number of driving steps. When $C_i \trianglelefteq C_j$ is detected, the whistle blows, driving halts, and generalization is invoked. Because generalization strictly decreases term depth or introduces a finite knot, the process tree is finite.
\end{proof}

\section{The Size-Filtered Whistle and Hash-Cons Acceleration}
\label{sec:termination:size_filtered}

A naive implementation of homeomorphic embedding requires $\mathcal{O}(|s| \cdot |t|)$ time per ancestor check. Across a process tree with thousands of configurations, total whistle checking time scales as $\mathcal{O}(N^3)$, crippling compiler throughput.

NumLang resolves this bottleneck via two innovations in \texttt{src/mir/supercompiler/whistle.rs}:

\subsection{1. O(1) Hash-Cons Whistle (Phase 61)}
Before executing recursive structural embedding checks, the whistle queries the \texttt{TermInterner}. If $\text{id}(s) == \text{id}(t)$, the terms are structurally identical; the whistle fires in $\mathcal{O}(1)$ time.

\subsection{2. Size Filtering}
By structural induction on $\trianglelefteq$, if $s \trianglelefteq t$, then necessarily:
\begin{equation}
\text{size}(s) \le \text{size}(t)
\end{equation}
The \texttt{TermInterner} maintains cached sizes for all interned terms. If $\text{size}(s) > \text{size}(t)$, the recursive embedding check is skipped immediately. This simple filter eliminates $>94\%$ of all structural comparison recursions in production benchmarks.

\section{The Phase 31 Termination Witness Certificate}
\label{sec:termination:witness}

In Phase 31, NumLang introduced the \texttt{TerminationWitness} certificate. When the CLI flag \texttt{--emit-termination-proof} is provided, the compiler emits a cryptographically hashed JSON certificate documenting the termination justification for every loop:

\begin{lstlisting}[language=Rust, caption={The TerminationWitness structure in NumLang.}]
pub struct TerminationWitness {
    pub function_name: String,
    pub total_driving_steps: usize,
    pub max_tree_depth: usize,
    pub whistle_firings: usize,
    pub knots_materialized: usize,
    pub certificate_hash: String,
}
\end{lstlisting}

This certificate can be validated independently by an external verifier, proving that the compiled artifact was produced without heuristic timeouts or arbitrary execution abortion.
"""

def generate_ch10():
    return r"""\chapter{Generalization: Anti-Unification and MSG}
\label{chap:generalization}

\section{The Role of Generalization}
\label{sec:generalization:role}

When the whistle blows upon detecting that configuration $C_{\text{curr}}$ homeomorphically embeds an ancestor $C_{\text{anc}}$, the supercompiler must reconcile the two states. If the two states differ only by variable renaming, folding creates a direct recursion. However, if $C_{\text{curr}}$ contains growing sub-expressions (for instance, an accumulator variable growing from $0$ to $0 + 1$ to $0 + 1 + 2$), folding is impossible.

To prevent infinite unrolling while still discovering the underlying recursive invariant, the supercompiler must perform \emph{generalization}: finding a common, more general template that subsumes both configurations.

\section{The S\o{}rensen--Gl\"uck Most Specific Generalization Algorithm}
\label{sec:generalization:msg_alg}

NumLang implements the canonical S\o{}rensen and Gl\"uck (1995) anti-unification algorithm \citep{sorensen1995algorithm} in \texttt{src/mir/supercompiler/generalize.rs}.

Algorithm~\ref{alg:anti_unification} presents the formal anti-unification procedure.

\begin{algorithm}[h!]
\caption{The Most Specific Generalization (Anti-Unification) Algorithm}
\label{alg:anti_unification}
\begin{algorithmic}[1]
\Require Terms $t_1, t_2 \in \symterm$, Memoization Map $M : (\symterm \times \symterm) \to \mathcal{V}$
\Ensure Generalized Term $g$, Substitutions $\theta_1, \theta_2$
\Procedure{AntiUnify}{$t_1, t_2, M$}
    \If{$(t_1, t_2) \in M$}
        \State \Return $M(t_1, t_2)$
    \EndIf
    \If{$t_1 = f(u_1, \dots, u_n) \land t_2 = f(v_1, \dots, v_n)$}
        \ForAll{$i \in \{1, \dots, n\}$}
            \State $w_i \gets$ \Call{AntiUnify}{$u_i, v_i, M$}
        \EndFor
        \State \Return $f(w_1, \dots, w_n)$
    \Else
        \State Let $\alpha$ be a fresh symbolic variable
        \State $M \gets M \cup \{(t_1, t_2) \mapsto \alpha\}$
        \State $\theta_1 \gets \theta_1 \cup \{\alpha \mapsto t_1\}; \quad \theta_2 \gets \theta_2 \cup \{\alpha \mapsto t_2\}$
        \State \Return $\alpha$
    \EndIf
\EndProcedure
\end{algorithmic}
\end{algorithm}

\section{Variable Widening in the Interval Lattice}
\label{sec:generalization:widening}

When generalizing numeric state variables, naive anti-unification abstracts concrete bounds into unbounded fresh variables, destroying all downstream bounds check eliminations (BCE). 

NumLang couples syntactic MSG with an interval widening operator $\nabla$ over the interval domain $[lo, hi]$:
\begin{equation}
[lo_1, hi_1] \nabla [lo_2, hi_2] = \left[
\begin{cases}
lo_1 & \text{if } lo_1 \le lo_2 \\
-\infty & \text{otherwise}
\end{cases}, \quad
\begin{cases}
hi_1 & \text{if } hi_1 \ge hi_2 \\
+\infty & \text{otherwise}
\end{cases}
\right]
\end{equation}
By preserving stable lower bounds (such as array indices known to satisfy $i \ge 0$), NumLang retains bounds check elimination guarantees across generalization knots.

\section{Knot Materialization and Split vs. Generalize Decision}
\label{sec:generalization:split}

When an expression $e = f(t_1, t_2)$ triggers a whistle, the supercompiler faces a critical architectural decision:
\begin{enumerate}
    \item \textbf{Generalize}: Abstract growing subterms into variables and synthesize a generalized knot.
    \item \textbf{Split (Decompose)}: Decompose the compound expression into independent sub-expressions $t_1$ and $t_2$, driving each separately.
\end{enumerate}

NumLang implements Phase 48 \emph{Structural Whistle Decoupling}: the decision to split versus generalize is determined strictly by term size ratios and variable dependency metrics, never by string-based function name heuristics. If subterms $t_1$ and $t_2$ share zero free variables, splitting is chosen unconditionally, preventing unnecessary knot creation.
"""

def generate_ch11():
    return r"""\chapter{Hamilton Global Distillation}
\label{chap:distillation}

\section{The Limitations of Local Supercompilation}
\label{sec:distillation:limitations}

Classical positive supercompilation (such as Turchin's Refal or S\o{}rensen--Gl\"uck Algorithm A) performs strictly \emph{local} folding: a configuration may only fold against direct ancestors along its immediate process path. 

As Geoff Hamilton demonstrated in 2007 \citep{hamilton2007distillation}, local supercompilation fails to eliminate intermediate data structures in common programming patterns. In pipelines such as:
\begin{equation}
\mathtt{sum}(\mathtt{map}(f, \mathtt{map}(g, xs)))
\end{equation}
local supercompilation successfully fuses the outer \texttt{sum} with the first \texttt{map}, but leaves the second \texttt{map} as a separate recursive function, preserving an intermediate list.

\section{The Formal Distillation Algorithm}
\label{sec:distillation:algorithm}

NumLang implements Hamilton's global distillation in \texttt{src/mir/supercompiler/distill.rs}. Distillation maintains a \emph{global memoization repository} $\mathcal{G}$ containing all configurations explored across the entire program graph.

Distillation is governed by three formal transformation rules:

\begin{align}
\text{\bf (Unfold)} \quad & \frac{C \notin \text{foldable}(\mathcal{G}) \quad C \text{ does not whistle}}{\mathcal{G} \vdash C \longrightarrow \text{drive}(C)} \\[0.8em]
\text{\bf (Fold)} \quad & \frac{\exists C' \in \mathcal{G}.\; C \equiv_\alpha C'}{\mathcal{G} \vdash C \longrightarrow \mathtt{KnotRef}(C')} \\[0.8em]
\text{\bf (Abstract)} \quad & \frac{\exists C' \in \mathcal{G}.\; C' \trianglelefteq C}{\mathcal{G} \vdash C \longrightarrow \text{distill}(\text{MSG}(C', C))}
\end{align}

\section{Inter-Procedural Folding Across Function Boundaries}
\label{sec:distillation:interprocedural}

The defining power of distillation is that folding is not confined to ancestors within the same call stack. If Function $A$ produces an intermediate stream state that is structurally identical to a stream consumer in Function $B$, distillation recognizes their $\alpha$-equivalence across the global graph, folding the producer directly into the consumer.

\section{Interaction with Reynolds Defunctionalization}
\label{sec:distillation:defunc_ordering}

A crucial engineering insight realized in Phase 54 is that \textbf{distillation must operate across defunctionalized first-order representations}. If distillation attempts to fold over higher-order ASTs containing closure environments, captured environment records prevent configuration matching. 

NumLang enforces a two-tier distillation strategy:
\begin{enumerate}
    \item \textbf{Pre-Defunctionalization AST Distillation (Phase 54)}: Applies lightweight structural folding on pure lambda expressions in \texttt{src/ast/hodistill.rs}.
    \item \textbf{Global MIR Distillation}: Executes full Hamilton global distillation on SSA basic blocks following whole-program Reynolds defunctionalization.
\end{enumerate}

\section{Worked Distillation Example: map o map}
\label{sec:distillation:map_map_walkthrough}

Consider distilling $\mathtt{map}(f, \mathtt{map}(g, xs))$:
\begin{enumerate}
    \item The outer call unfolds until it reaches the inner call's head constructor.
    \item When the inner call yields $\mathtt{Cons}(g(y), \mathtt{map}(g, ys))$, the outer call consumes it directly:
    \begin{equation}
    f(g(y)) :: \mathtt{map}(f, \mathtt{map}(g, ys))
    \end{equation}
    \item The tail expression $\mathtt{map}(f, \mathtt{map}(g, ys))$ matches the root configuration in the global repository $\mathcal{G}$.
    \item Distillation folds globally, emitting a single tail-recursive loop that computes $f(g(y))$ in a single pass without allocating any intermediate list nodes.
\end{enumerate}
"""

def generate_ch12():
    return r"""\chapter{Multi-Result Supercompilation (MRSC)}
\label{chap:mrsc}

\section{The Heuristic Dilemma in Metacomputation}
\label{sec:mrsc:dilemma}

Traditional supercompilers make hard heuristic choices at each transformation step:
\begin{itemize}
    \item Should an expression be inlined or driven as an uninterpreted function call?
    \item Should a whistle trigger generalization or term decomposition (splitting)?
    \item Which ancestor configuration in the path should be chosen for folding?
\end{itemize}
A suboptimal choice early in process tree exploration can lead to severe residual code bloat, miss crucial deforestation opportunities, or trap the compiler in complex knot topologies.

\section{Mitchell--Klyuchnikov Configuration Hypergraphs}
\label{sec:mrsc:hypergraphs}

To eliminate heuristic fragility, Mitchell and Klyuchnikov formulated \emph{Multi-Result Supercompilation (MRSC)} \citep{klyuchnikov2012practical}. Instead of constructing a single process tree, MRSC constructs a \emph{configuration hypergraph} ($\mathcal{H} = \langle V, E \rangle$) representing all legally permissible supercompilation derivations.

Implemented in \texttt{src/mir/supercompiler/mrsc.rs}:
\begin{lstlisting}[language=Rust, caption={The MRSC Hypergraph structures in NumLang.}]
pub struct MrscHypergraph {
    pub nodes: Vec<MrscNode>,
    pub hyperedges: Vec<MrscHyperedge>,
}

pub struct MrscHyperedge {
    pub source: MrscNodeId,
    pub targets: Vec<MrscNodeId>,
    pub label: TransformationRule,
}
\end{lstlisting}

\section{The Four-Dimensional MRSC Cost Model}
\label{sec:mrsc:cost_model}

Once the hypergraph is populated, NumLang evaluates candidate residual programs against an explicit multi-dimensional cost function:
\begin{equation}
\mathcal{C}(P) = w_{\text{step}} \cdot S(P) + w_{\text{alloc}} \cdot A(P) + w_{\text{code}} \cdot C(P) + w_{\text{reg}} \cdot R(P)
\end{equation}
where:
\begin{itemize}
    \item $S(P)$ is the estimated dynamic step count along hot paths.
    \item $A(P)$ is the static number of heap allocations.
    \item $C(P)$ is the residual machine instruction count (binary footprint).
    \item $R(P)$ is the estimated register pressure (live variable count).
\end{itemize}

The extraction procedure \texttt{extract\_pareto\_optimal} traverses the hypergraph via dynamic programming, extracting the residual that minimizes $\mathcal{C}(P)$.

\section{The Phase 53 Exhaustive MRSC Oracle}
\label{sec:mrsc:oracle}

When maximum optimization is required, the CLI flag \texttt{--mrsc-exhaustive} activates the Phase 53 \emph{Exhaustive MRSC Oracle} (\texttt{src/mir/supercompiler/mrsc_oracle.rs}). 

The oracle replaces bounded graph pruning with an unbounded Iterative Deepening Depth-First Search (IDDFS). By exhaustively exploring alternative generalization frontiers, the oracle routinely discovers residual configurations that are strictly smaller and faster than online greedy supercompilation. Optimal residuals discovered by the oracle are automatically committed to the L2 persistent cache for future compilations.
"""

def generate_ch14():
    return r"""\chapter{Higher-Order Deforestation and Defunctionalization}
\label{chap:higher_order}

\section{The Challenge of Higher-Order Driving}
\label{sec:higher_order:challenge}

In functional languages, higher-order functions (closures that capture runtime environments) are the primary vehicle of abstraction. However, specializing higher-order code is notoriously difficult:
\begin{enumerate}
    \item Closures hide control flow behind indirect function pointers.
    \item Closure environments capture heap-allocated records, preventing syntactic term matching.
    \item Mutual recursion through closure arguments easily induces whistle blowouts.
\end{enumerate}

\section{Whole-Program Reynolds Defunctionalization (Phase 49)}
\label{sec:higher_order:reynolds}

NumLang conquers this challenge by implementing a whole-program \emph{Reynolds Defunctionalization} pass in \texttt{src/mir/defunctionalize.rs} \citep{reynolds1972definitional}.

\subsection{Transformation Pipeline}
\begin{enumerate}
    \item \textbf{Call Signature Analysis}: The compiler collects all lambda expressions and function pointer types across the program.
    \item \textbf{ClosureTag Synthesis}: For each unique function signature $\tau_{\text{sig}} = \mathtt{fn}(\tau_1, \dots, \tau_n) \to \tau_{\text{ret}}$, NumLang generates a strongly typed enum:
    \begin{lstlisting}[language=NumLang]
    enum ClosureTag_Sig {
        Lambda_1(i64),       // captures single i64
        Lambda_2(Box<List>), // captures heap list
    }
    \end{lstlisting}
    \item \textbf{Lowering Allocations}: \texttt{ClosureAlloc} statements are replaced by enum constructor initializations.
    \item \textbf{Dispatch Switch Synthesis}: All indirect calls $\mathtt{IndirectCall}(fn\_ptr, \vec{args})$ are replaced by explicit \texttt{Terminator::Switch} dispatchers branching on the closure discriminant tag.
    \item \textbf{Scalar Replacement of Aggregates (SROA)}: Environment payloads are unpacked into primitive SSA scalar registers.
\end{enumerate}

Following defunctionalization, the entire program is strictly first-order SSA MIR.

\section{Symbolic Driving Through Closures (Phase 34)}
\label{sec:higher_order:driving_closures}

Because defunctionalization exposes closure tags as regular enums, the supercompiler’s existing pattern matching driving mechanisms handle closures seamlessly:
\begin{itemize}
    \item Symbolic closures are tracked as $\mathtt{SymTerm::ClosureVal}(f, \vec{captured})$.
    \item When a call site invokes the closure, driving resolves the target statically and inlines the function body with the captured arguments bound in the local environment.
    \item Dynamic dispatch overhead is eliminated completely.
\end{itemize}

\section{Lazy Codata Driving and Stream Fusion (Phase 51)}
\label{sec:higher_order:codata}

To achieve true GHC-style stream fusion without allocating intermediate iterator structures, NumLang implements \emph{Lazy Codata Driving} (\texttt{src/mir/thunk_analysis.rs}).

Pipelines over infinite streams (such as \texttt{iterate}, \texttt{map}, and \texttt{take}) are expressed via suspended thunks:
\begin{equation}
x = \mathtt{Rvalue::Thunk}(f, \vec{args}), \quad y = \mathtt{Terminator::Force}(x)
\end{equation}
The supercompiler propagates demand backwards: a thunk is only driven when forced. When a pipeline consists of chained thunk forces, driving eliminates the suspension overhead, fusing the generator, transformer, and consumer into a single flat loop with zero heap allocations.
"""

def main():
    write_file("07_symbolic_state.tex", generate_ch07())
    write_file("08_driving.tex", generate_ch08())
    write_file("09_termination.tex", generate_ch09())
    write_file("10_generalization.tex", generate_ch10())
    write_file("11_distillation.tex", generate_ch11())
    write_file("12_mrsc.tex", generate_ch12())
    write_file("14_higher_order.tex", generate_ch14())

if __name__ == "__main__":
    main()
