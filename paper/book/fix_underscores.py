#!/usr/bin/env python3
"""
fix_underscores.py: Escapes unescaped underscores outside math mode and listings in all .tex files.
"""

import glob
import re

def escape_underscores_in_text(text):
    # Split by listings blocks
    parts = re.split(r'(\\begin\{lstlisting\}.*?\\end\{lstlisting\})', text, flags=re.DOTALL)
    for i in range(len(parts)):
        if parts[i].startswith(r'\begin{lstlisting}'):
            continue
        
        # Now we process parts[i] character by character or chunk by chunk
        # Math environments:
        # $ ... $
        # $$ ... $$
        # \[ ... \]
        # \begin{equation} ... \end{equation}
        # \begin{align} ... \end{align}
        # \begin{align*} ... \end{align*}
        
        res = []
        pos = 0
        s = parts[i]
        n = len(s)
        in_math = False
        math_end = None
        
        while pos < n:
            # Check math delimiters if not in math
            if not in_math:
                if s.startswith('$$', pos):
                    in_math = True
                    math_end = '$$'
                    res.append('$$')
                    pos += 2
                    continue
                elif s.startswith('$', pos):
                    in_math = True
                    math_end = '$'
                    res.append('$')
                    pos += 1
                    continue
                elif s.startswith(r'\[', pos):
                    in_math = True
                    math_end = r'\]'
                    res.append(r'\[')
                    pos += 2
                    continue
                elif s.startswith(r'\begin{equation}', pos):
                    in_math = True
                    math_end = r'\end{equation}'
                    res.append(r'\begin{equation}')
                    pos += len(r'\begin{equation}')
                    continue
                elif s.startswith(r'\begin{align}', pos):
                    in_math = True
                    math_end = r'\end{align}'
                    res.append(r'\begin{align}')
                    pos += len(r'\begin{align}')
                    continue
                elif s.startswith(r'\begin{align*}', pos):
                    in_math = True
                    math_end = r'\end{align*}'
                    res.append(r'\begin{align*}')
                    pos += len(r'\begin{align*}')
                    continue
                
                # Check underscore in text mode
                if s[pos] == '_':
                    # Check if preceded by backslash
                    if pos > 0 and s[pos-1] == '\\':
                        res.append('_')
                    else:
                        res.append(r'\_')
                    pos += 1
                else:
                    res.append(s[pos])
                    pos += 1
            else:
                # We are in math mode, check if ending
                if s.startswith(math_end, pos):
                    in_math = False
                    res.append(math_end)
                    pos += len(math_end)
                    math_end = None
                else:
                    # In math mode, wait: what if inside \mathtt{foo_bar} or \text{foo_bar}?
                    # In LaTeX math mode, \text{...} or \mathtt{...} with underscore can also cause issues if not escaped!
                    # But standard math subscript x_i is normal.
                    res.append(s[pos])
                    pos += 1
                    
        parts[i] = "".join(res)
        
    return "".join(parts)

def main():
    for f in sorted(glob.glob('paper/book/chapters/*.tex')):
        with open(f, 'r', encoding='utf-8') as fp:
            content = fp.read()
        fixed = escape_underscores_in_text(content)
        if fixed != content:
            with open(f, 'w', encoding='utf-8') as fp:
                fp.write(fixed)
            print(f"Fixed underscores in {f}")

if __name__ == '__main__':
    main()
