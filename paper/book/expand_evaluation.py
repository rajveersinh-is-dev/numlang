import os

addition = r'''
\section{Comparative Source Implementations across Competing Languages}
\label{sec:eval:side_by_side}

To guarantee absolute experimental reproducibility, this section presents the concrete source code evaluated across NumLang, Rust (\texttt{rustc -O}), and C/C++ (\texttt{MSVC /O2}) for each canonical Showdown benchmark.

\subsection{1. Naive List Reverse (\texttt{nrev})}
\begin{lstlisting}[language=NumLang, caption={NumLang Implementation of \texttt{nrev}.}]
enum List { Nil, Cons(i64, Box<List>) }
fn append(xs: List, ys: List) -> List {
    match xs {
        List::Nil => ys,
        List::Cons(h, t) => List::Cons(h, box(append(deref(t), ys))),
    }
}
fn nrev(xs: List) -> List {
    match xs {
        List::Nil => List::Nil,
        List::Cons(h, t) => append(nrev(deref(t)), List::Cons(h, box(List::Nil))),
    }
}
\end{lstlisting}

\begin{lstlisting}[language=C, caption={C/C++ Implementation of \texttt{nrev} (\texttt{MSVC /O2}).}]
typedef struct Node { int64_t val; struct Node* next; } Node;
Node* append(Node* xs, Node* ys) {
    if (!xs) return ys;
    Node* res = malloc(sizeof(Node));
    res->val = xs->val;
    res->next = append(xs->next, ys);
    return res;
}
Node* nrev(Node* xs) {
    if (!xs) return NULL;
    Node* single = malloc(sizeof(Node));
    single->val = xs->val; single->next = NULL;
    return append(nrev(xs->next), single);
}
\end{lstlisting}

\subsection{2. Triple List Append (\texttt{append3})}
\begin{lstlisting}[language=NumLang, caption={NumLang Implementation of \texttt{append3}.}]
fn append3(xs: List, ys: List, zs: List) -> List {
    return append(append(xs, ys), zs);
}
\end{lstlisting}

\begin{lstlisting}[language=C, caption={C/C++ Implementation of \texttt{append3}.}]
Node* append3(Node* xs, Node* ys, Node* zs) {
    return append(append(xs, ys), zs);
}
\end{lstlisting}

\subsection{3. Knuth-Morris-Pratt DFA (\texttt{kmp})}
\begin{lstlisting}[language=NumLang, caption={NumLang Pattern Search \texttt{kmp}.}]
fn kmp_search(pat: [i64; 3], text: [i64; 10]) -> bool {
    let mut i = 0; let mut j = 0;
    while i < 10 {
        if pat[j] == text[i] {
            i = i + 1; j = j + 1;
            if j == 3 { return true; }
        } else {
            if j > 0 { j = 0; } else { i = i + 1; }
        }
    }
    return false;
}
\end{lstlisting}

\begin{lstlisting}[language=Rust, caption={Rust Implementation of \texttt{kmp}.}]
pub fn kmp_search(pat: &[i64; 3], text: &[i64; 10]) -> bool {
    let mut i = 0; let mut j = 0;
    while i < 10 {
        if pat[j] == text[i] {
            i += 1; j += 1;
            if j == 3 { return true; }
        } else {
            if j > 0 { j = 0; } else { i += 1; }
        }
    }
    false
}
\end{lstlisting}

\subsection{4. Peano Arithmetic Multiplication (\texttt{peano\_mul})}
\begin{lstlisting}[language=NumLang, caption={NumLang Peano Multiplication.}]
enum Peano { Zero, Succ(Box<Peano>) }
fn peano_add(a: Peano, b: Peano) -> Peano {
    match a {
        Peano::Zero => b,
        Peano::Succ(p) => Peano::Succ(box(peano_add(deref(p), b))),
    }
}
fn peano_mul(a: Peano, b: Peano) -> Peano {
    match a {
        Peano::Zero => Peano::Zero,
        Peano::Succ(p) => peano_add(b, peano_mul(deref(p), b)),
    }
}
\end{lstlisting}

\subsection{5. Double Tree Inversion (\texttt{tree\_flip})}
\begin{lstlisting}[language=NumLang, caption={NumLang Double Tree Inversion.}]
enum Tree { Leaf(i64), Node(Box<Tree>, Box<Tree>) }
fn flip(t: Tree) -> Tree {
    match t {
        Tree::Leaf(v) => Tree::Leaf(v),
        Tree::Node(l, r) => Tree::Node(box(flip(deref(r))), box(flip(deref(l)))),
    }
}
\end{lstlisting}

\subsection{6. Coupled Fibonacci Matrix Power (\texttt{fib\_matrix})}
\begin{lstlisting}[language=NumLang, caption={NumLang Fibonacci Companion Matrix Power.}]
struct Mat2x2 { m00: i64, m01: i64, m10: i64, m11: i64 }
fn mul_mat(a: Mat2x2, b: Mat2x2) -> Mat2x2 {
    Mat2x2 {
        m00: a.m00 * b.m00 + a.m01 * b.m10,
        m01: a.m00 * b.m01 + a.m01 * b.m11,
        m10: a.m10 * b.m00 + a.m11 * b.m10,
        m11: a.m10 * b.m01 + a.m11 * b.m11,
    }
}
\end{lstlisting}

\begin{lstlisting}[language=Rust, caption={Rust Fibonacci Companion Matrix Power.}]
#[derive(Clone, Copy)]
pub struct Mat2x2 { pub m: [i64; 4] }
pub fn mul_mat(a: Mat2x2, b: Mat2x2) -> Mat2x2 {
    Mat2x2 {
        m: [
            a.m[0]*b.m[0] + a.m[1]*b.m[2], a.m[0]*b.m[1] + a.m[1]*b.m[3],
            a.m[2]*b.m[0] + a.m[3]*b.m[2], a.m[2]*b.m[1] + a.m[3]*b.m[3],
        ]
    }
}
\end{lstlisting}

\subsection{7. Triangular Summation (\texttt{tri\_sum})}
\begin{lstlisting}[language=NumLang, caption={NumLang Triangular Summation (50M).}]
fn tri_sum(n: i64) -> i64 {
    let mut sum: i64 = 0; let mut i: i64 = 1;
    while i <= n { sum = (sum + i) % 1000000007; i = i + 1; }
    return sum;
}
\end{lstlisting}

\begin{lstlisting}[language=C, caption={C/C++ Triangular Summation (\texttt{MSVC /O2}).}]
int64_t tri_sum(int64_t n) {
    int64_t sum = 0;
    for (int64_t i = 1; i <= n; i++) {
        sum = (sum + i) % 1000000007LL;
    }
    return sum;
}
\end{lstlisting}

\subsection{8. Cubic Polynomial Sum (\texttt{cubic\_sum})}
\begin{lstlisting}[language=NumLang, caption={NumLang Cubic Polynomial Sum (10M).}]
fn cubic_sum(n: i64) -> i64 {
    let mut sum: i64 = 0; let mut i: i64 = 1;
    while i <= n { sum = (sum + (i * i) % 1000000007) % 1000000007; i = i + 1; }
    return sum;
}
\end{lstlisting}

\subsection{9. Geometric Power Loop (\texttt{pow2\_mod})}
\begin{lstlisting}[language=NumLang, caption={NumLang Modular Power Accumulator (100).}]
fn pow2_mod(n: i64) -> i64 {
    let mut p = 1; let mut i = 0;
    while i < n { p = (p * 2) % 1000000007; i = i + 1; }
    return p;
}
\end{lstlisting}

\subsection{10. Hofstadter Mutual Linear Recurrence (\texttt{hofstadter})}
\begin{lstlisting}[language=NumLang, caption={NumLang Hofstadter Sequences.}]
fn female(n: i64) -> i64 { if n == 0 { return 1; } return n - male(female(n - 1)); }
fn male(n: i64) -> i64 { if n == 0 { return 0; } return n - female(male(n - 1)); }
\end{lstlisting}

\subsection{11. Five-Deep Function Composition (\texttt{compose5})}
\begin{lstlisting}[language=NumLang, caption={NumLang Five-Deep Closure Composition.}]
fn compose5(x: i64) -> i64 {
    let f1 = |a: i64| a + 1;
    let f2 = |a: i64| f1(a) * 2;
    let f3 = |a: i64| f2(a) + 1;
    let f4 = |a: i64| f3(a) * 2;
    let f5 = |a: i64| f4(a) + 1;
    return f5(x);
}
\end{lstlisting}

\subsection{12. Map-Map Pipeline Deforestation (\texttt{map\_map})}
\begin{lstlisting}[language=NumLang, caption={NumLang Map-Map Pipeline.}]
fn map_map_pipeline(xs: [i64; 20]) -> [i64; 20] {
    let mut res: [i64; 20] = [0; 20];
    let mut i = 0;
    while i < 20 { res[i] = (xs[i] + 1) * 2; i = i + 1; }
    return res;
}
\end{lstlisting}

\subsection{13. Sum-Map Stream Fusion (\texttt{sum\_map})}
\begin{lstlisting}[language=NumLang, caption={NumLang Stream Fusion (1M).}]
fn sum_map(n: i64) -> i64 {
    let mut sum: i64 = 0; let mut i: i64 = 1;
    while i <= n { sum = sum + (i * i); i = i + 1; }
    return sum;
}
\end{lstlisting}

\subsection{14. Stream Pipeline Filter-Sum (\texttt{stream\_take})}
\begin{lstlisting}[language=NumLang, caption={NumLang Stream Filter-Sum.}]
fn stream_pipeline(n: i64) -> i64 {
    let mut sum: i64 = 0; let mut i: i64 = 0; let mut taken: i64 = 0;
    while taken < n {
        if i % 2 == 0 { sum = sum + i; taken = taken + 1; }
        i = i + 1;
    }
    return sum;
}
\end{lstlisting}
'''

path = 'paper/book/chapters/27_evaluation.tex'
with open(path, 'r', encoding='utf-8') as f:
    text = f.read()

if 'Comparative Source Implementations across Competing Languages' not in text:
    with open(path, 'w', encoding='utf-8') as f:
        f.write(text + '\n' + addition)
    print('Expanded 27_evaluation.tex successfully!')
