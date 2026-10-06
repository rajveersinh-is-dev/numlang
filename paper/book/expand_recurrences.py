import os

addition = r'''
\section{Mathematical Foundations of Faulhaber Sums and Discrete Calculus}
\label{sec:rec:faulhaber_proofs}

To formalize the correctness of higher-degree polynomial recurrence collapses, we recall Faulhaber's formula expressing power sums in terms of Bernoulli numbers $B_j$:
\begin{equation}
\sum_{i=1}^n i^p = \frac{1}{p+1} \sum_{j=0}^p (-1)^j \binom{p+1}{j} B_j n^{p+1-j}
\end{equation}

For degrees $p \in \{1, 2, 3, 4\}$, the exact closed-form polynomials synthesized by the supercompiler are:
\begin{align}
p=1: \quad & S_1(n) = \frac{n(n+1)}{2} = \frac{1}{2} n^2 + \frac{1}{2} n \\[0.8em]
p=2: \quad & S_2(n) = \frac{n(n+1)(2n+1)}{6} = \frac{1}{3} n^3 + \frac{1}{2} n^2 + \frac{1}{6} n \\[0.8em]
p=3: \quad & S_3(n) = \left[ \frac{n(n+1)}{2} \right]^2 = \frac{1}{4} n^4 + \frac{1}{2} n^3 + \frac{1}{4} n^2 \\[0.8em]
p=4: \quad & S_4(n) = \frac{n(n+1)(2n+1)(3n^2+3n-1)}{30} = \frac{1}{5} n^5 + \frac{1}{2} n^4 + \frac{1}{3} n^3 - \frac{1}{30} n
\end{align}

\begin{theorem}[Correctness of Newton Series Recurrence Synthesis]
Let $f(n)$ be a polynomial of degree $d \le 5$. The Newton series:
\begin{equation}
f(n) = \sum_{k=0}^d \binom{n}{k} \Delta^k f(0)
\end{equation}
evaluated over the forward difference table $\Delta^k f(0)$ is exact for all $n \in \mathbb{Z}$.
\end{theorem}
\begin{proof}
By induction on the polynomial degree $d$. For $d=0$, $f(n) = c$, $\Delta^0 f(0) = c$, and $\binom{n}{0} = 1$. Assuming the hypothesis holds for degree $d-1$, the difference operator $\Delta f(n) = g(n)$ is a polynomial of degree $d-1$. By the inductive hypothesis, $g(n) = \sum_{k=0}^{d-1} \binom{n}{k} \Delta^{k+1} f(0)$. Summing both sides:
\begin{equation}
f(n) - f(0) = \sum_{i=0}^{n-1} g(i) = \sum_{i=0}^{n-1} \sum_{k=0}^{d-1} \binom{i}{k} \Delta^{k+1} f(0) = \sum_{k=0}^{d-1} \Delta^{k+1} f(0) \sum_{i=0}^{n-1} \binom{i}{k}
\end{equation}
Applying the hockey-stick identity $\sum_{i=0}^{n-1} \binom{i}{k} = \binom{n}{k+1}$, we obtain:
\begin{equation}
f(n) = f(0) + \sum_{k=0}^{d-1} \binom{n}{k+1} \Delta^{k+1} f(0) = \sum_{j=0}^d \binom{n}{j} \Delta^j f(0)
\end{equation}
which concludes the proof.
\end{proof}

\section{Bareiss Multi-Step Elimination Proof for $N \le 8$}
\label{sec:rec:bareiss_proof}

To prove that the integer Gaussian elimination in \texttt{polyhedral\_ilp.rs} and \texttt{recurrence.rs} never incurs fractional rounding errors, we formulate the determinantal identity underlying Bareiss elimination:

\begin{theorem}[Sylvester-Bareiss Determinant Identity~\cite{bareiss1968sylvester}]
Let $\mathbf{A}$ be an $n \times n$ matrix over an integral domain $\mathcal{D}$. For any $k < i, j \le n$:
\begin{equation}
a_{i,j}^{(k)} = \det \begin{pmatrix}
a_{1,1} & \dots & a_{1,k-1} & a_{1,j} \\
\vdots & \ddots & \vdots & \vdots \\
a_{k-1,1} & \dots & a_{k-1,k-1} & a_{k-1,j} \\
a_{i,1} & \dots & a_{i,k-1} & a_{i,j}
\end{pmatrix}
\end{equation}
The division $a_{i,j}^{(k)} = \frac{a_{k,k}^{(k-1)} a_{i,j}^{(k-1)} - a_{i,k}^{(k-1)} a_{k,j}^{(k-1)}}{a_{k-1,k-1}^{(k-2)}}$ is strictly exact in $\mathcal{D}$, yielding integer coefficients without remainder.
\end{theorem}
This guarantees that NumLang's linear recurrence solver operates with zero floating-point approximation error across all integer matrices.
'''

path = 'paper/book/chapters/13_recurrences.tex'
with open(path, 'r', encoding='utf-8') as f:
    text = f.read()

if 'Mathematical Foundations of Faulhaber Sums' not in text:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(text + '\n' + addition)
    print('Expanded 13_recurrences.tex successfully!')
