import glob

replacements = {
    '\u2014': '---', # em dash
    '\u2013': '--',  # en dash
    '\u201c': '``',  # left double quote
    '\u201d': "''",  # right double quote
    '\u2018': '`',   # left single quote
    '\u2019': "'",   # right single quote
}

for f in glob.glob('paper/book/chapters/*.tex'):
    with open(f, 'r', encoding='utf-8') as fp:
        content = fp.read()
    orig = content
    for k, v in replacements.items():
        content = content.replace(k, v)
    if content != orig:
        with open(f, 'w', encoding='utf-8') as fp:
            fp.write(content)
        print(f'Sanitized typography in {f}')
