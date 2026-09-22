#!/usr/bin/env python3
"""
Genera la versión PDF de la documentación en Markdown del proyecto.

El PDF es un artefacto derivado: nunca se edita a mano. Se edita el .md y se
regenera con este script.

Uso:
    python3 scripts/build-pdf.py docs/requerimientos-funcionales.md

Requiere XeLaTeX (TeXLive / MacTeX). No requiere conexión a internet.
"""

from __future__ import annotations

import math
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

# --------------------------------------------------------------------------
# Escape de caracteres reservados de LaTeX
# --------------------------------------------------------------------------

_ESCAPES = [
    ("\\", r"\textbackslash{}"),
    ("&", r"\&"),
    ("%", r"\%"),
    ("$", r"\$"),
    ("#", r"\#"),
    ("_", r"\_"),
    ("{", r"\{"),
    ("}", r"\}"),
    ("~", r"\textasciitilde{}"),
    ("^", r"\textasciicircum{}"),
]

_CODE_TOKEN = "\x00CODE{}\x00"


def escape(text: str) -> str:
    for src, dst in _ESCAPES:
        text = text.replace(src, dst)
    return text


def inline(text: str) -> str:
    """Convierte marcado inline de Markdown a LaTeX."""
    # 1. Se apartan los spans de código para que el escape no los altere.
    codes: list[str] = []

    def stash(m: re.Match[str]) -> str:
        codes.append(m.group(1))
        return _CODE_TOKEN.format(len(codes) - 1)

    text = re.sub(r"`([^`]+)`", stash, text)

    # 2. Escape del texto normal.
    text = escape(text)

    # 3. Énfasis. Negrita antes que cursiva para no romper los dobles asteriscos.
    text = re.sub(r"\*\*(.+?)\*\*", r"\\textbf{\1}", text)
    text = re.sub(r"(?<!\*)\*([^*]+?)\*(?!\*)", r"\\emph{\1}", text)

    # 4. Reinserción del código ya escapado.
    for i, code in enumerate(codes):
        text = text.replace(
            _CODE_TOKEN.format(i),
            r"\codeinline{" + escape(code) + "}",
        )
    return text


# --------------------------------------------------------------------------
# Tablas
# --------------------------------------------------------------------------

_PRIO_COLORS = {"M": "prioM", "S": "prioS", "C": "prioC", "W": "prioW"}


def split_row(line: str) -> list[str]:
    return [c.strip() for c in line.strip().strip("|").split("|")]


def column_widths(header: list[str], rows: list[list[str]]) -> list[float]:
    """Reparte el ancho según la raíz del largo del contenido.

    La raíz comprime las diferencias: sin ella una columna de descripciones
    largas se comería todo el ancho y dejaría las columnas de ID ilegibles.
    """
    weights: list[float] = []
    for i, head in enumerate(header):
        lengths = [len(r[i]) for r in rows if i < len(r)]
        avg = sum(lengths) / len(lengths) if lengths else 1.0
        weights.append(math.sqrt(max(avg, len(head), 1.0)))
    total = sum(weights)
    return [w / total for w in weights]


#: Caracteres que caben en una línea que ocupara el ancho completo del texto.
_CHARS_PER_TEXTWIDTH = 105

#: Altura, en líneas, a partir de la cual se considera que una tabla no cabe
#: holgadamente en una página y conviene permitir que se parta.
_SHORT_TABLE_LINES = 34


def estimate_height(rows: list[list[str]], widths: list[float]) -> int:
    """Altura aproximada de la tabla en líneas de texto."""
    total = 0
    for row in rows:
        tallest = 1
        for i, frac in enumerate(widths):
            if i >= len(row):
                continue
            capacity = max(frac * _CHARS_PER_TEXTWIDTH, 1)
            tallest = max(tallest, math.ceil(len(row[i]) / capacity))
        total += tallest
    return math.ceil(total * 1.2) + 3  # interlineado de tabla + encabezado


def render_table(header: list[str], rows: list[list[str]]) -> str:
    ncols = len(header)
    widths = column_widths(header, rows)
    has_prio = header[-1].strip().rstrip(".").lower() in ("prio", "prioridad")

    # Una tabla que cabe en una página no debe declarar encabezado de
    # continuación: si se partiera igualmente, longtable puede dejar ese
    # encabezado y la línea de cierre solos al inicio de la página siguiente.
    # Se reserva espacio para ella y se emite sin \endhead.
    height = estimate_height(rows, widths)
    fits_in_page = height <= _SHORT_TABLE_LINES

    spec = []
    for i, frac in enumerate(widths):
        lengths = [len(r[i]) for r in rows if i < len(r)]
        avg = sum(lengths) / len(lengths) if lengths else 0
        kind = "C" if avg <= 6 else "L"
        spec.append(f"{kind}{{\\dimexpr {frac:.4f}\\textwidth-2\\tabcolsep\\relax}}")
    colspec = "@{}" + "".join(spec) + "@{}"

    head_cells = " & ".join(r"\thead{" + inline(h) + "}" for h in header)

    out = [
        r"\begingroup",
        r"\tablefont",
        r"\setlength{\extrarowheight}{2pt}",
        r"\rowcolors{2}{}{tablealt}",
    ]
    if fits_in_page:
        out.append(r"\needspace{" + str(height) + r"\baselineskip}")
    out += [
        r"\begin{longtable}{" + colspec + "}",
        r"\toprule",
        head_cells + r" \\",
        r"\midrule",
        r"\endfirsthead",
    ]
    if not fits_in_page:
        out += [
            r"\toprule",
            head_cells + r" \\",
            r"\midrule",
            r"\endhead",
        ]
    out += [
        r"\bottomrule",
        r"\endlastfoot",
    ]

    for n, row in enumerate(rows):
        cells = []
        for i in range(ncols):
            raw = row[i] if i < len(row) else ""
            if has_prio and i == ncols - 1 and raw in _PRIO_COLORS:
                cells.append(r"\prio{" + _PRIO_COLORS[raw] + "}{" + raw + "}")
            else:
                cells.append(inline(raw))
        # «\\*» prohíbe el salto de página tras las dos últimas filas. Sin esto
        # longtable puede dejar la línea de cierre —y con ella el encabezado
        # repetido— sola en la página siguiente.
        sep = r" \\*" if n >= len(rows) - 2 else r" \\"
        out.append(" & ".join(cells) + sep)

    out += [r"\end{longtable}", r"\endgroup", ""]
    return "\n".join(out)


# --------------------------------------------------------------------------
# Conversión del cuerpo del documento
# --------------------------------------------------------------------------

_HEADING_NUM = re.compile(r"^(\d+(?:\.\d+)*)[.\s]\s*")


def strip_manual_number(title: str) -> str:
    """Quita la numeración escrita a mano: LaTeX numera las secciones."""
    return _HEADING_NUM.sub("", title).strip()


def convert_body(lines: list[str]) -> str:
    out: list[str] = []
    i = 0
    first_section = True

    while i < len(lines):
        line = lines[i]
        stripped = line.strip()

        # Bloque de código cercado
        if stripped.startswith("```"):
            i += 1
            block: list[str] = []
            while i < len(lines) and not lines[i].strip().startswith("```"):
                block.append(lines[i].rstrip())
                i += 1
            i += 1
            out.append(r"\begin{codeblock}")
            out.extend(block)
            out.append(r"\end{codeblock}")
            out.append("")
            continue

        # Tabla
        if stripped.startswith("|") and i + 1 < len(lines) and re.match(
            r"^\s*\|[\s:|-]+\|\s*$", lines[i + 1]
        ):
            header = split_row(stripped)
            i += 2
            rows: list[list[str]] = []
            while i < len(lines) and lines[i].strip().startswith("|"):
                rows.append(split_row(lines[i]))
                i += 1
            out.append(render_table(header, rows))
            continue

        # Encabezados
        if stripped.startswith("### "):
            title = strip_manual_number(stripped[4:])
            out.append(r"\needspace{5\baselineskip}")
            out.append(r"\subsection{" + inline(title) + "}")
            i += 1
            continue

        if stripped.startswith("## "):
            title = strip_manual_number(stripped[3:])
            if not first_section:
                out.append(r"\clearpage")
            first_section = False
            out.append(r"\section{" + inline(title) + "}")
            i += 1
            continue

        # Lista con viñetas
        if stripped.startswith("- "):
            out.append(r"\begin{itemize}")
            while i < len(lines) and lines[i].strip().startswith("- "):
                out.append(r"\item " + inline(lines[i].strip()[2:]))
                i += 1
            out.append(r"\end{itemize}")
            out.append("")
            continue

        # Separador horizontal: las secciones ya dan separación suficiente.
        if stripped in ("---", "***", "___"):
            i += 1
            continue

        # Línea completamente en negrita: subtítulo suelto
        m = re.fullmatch(r"\*\*(.+?)\*\*", stripped)
        if m:
            out.append(r"\blocktitle{" + inline(m.group(1)) + "}")
            i += 1
            continue

        # Párrafo
        if stripped:
            para = [stripped]
            i += 1
            while i < len(lines) and lines[i].strip() and not re.match(
                r"^\s*(\||#{2,3}\s|-\s|```|\*\*[^*]+\*\*\s*$|---\s*$)", lines[i]
            ):
                para.append(lines[i].strip())
                i += 1
            out.append(inline(" ".join(para)))
            out.append("")
            continue

        i += 1

    return "\n".join(out)


# --------------------------------------------------------------------------
# Portada
# --------------------------------------------------------------------------

def build_cover(title: str, meta_lines: list[str]) -> str:
    meta: list[tuple[str, str]] = []
    history: list[tuple[str, str]] = []

    for raw in meta_lines:
        m = re.match(r"\*\*(.+?):\*\*\s*(.*)", raw.strip())
        if not m:
            continue
        key, value = m.group(1).strip(), m.group(2).strip()
        if key.lower().startswith("cambios"):
            history.append((key, value))
        else:
            meta.append((key, value))

    # "Proyecto — Subtítulo" se separa en dos niveles tipográficos.
    if "—" in title:
        main, _, subtitle = title.partition("—")
        main, subtitle = main.strip(), subtitle.strip()
    else:
        main, subtitle = title.strip(), ""

    parts = [
        r"\begin{titlepage}",
        r"\centering",
        r"\vspace*{5cm}",
        r"\coverrule\par",
        r"\vspace{1.15cm}",
        r"{\fontsize{34}{40}\selectfont\bfseries\color{ink}" + inline(main) + r"\par}",
    ]
    if subtitle:
        parts += [
            r"\vspace{0.5cm}",
            r"{\fontsize{17}{22}\selectfont\color{accent}" + inline(subtitle) + r"\par}",
        ]
    parts += [
        r"\vspace{1.15cm}",
        r"\coverrule\par",
        r"\vspace{2.6cm}",
        r"\begin{tabular}{@{}r@{\hspace{1.2em}}L{8.5cm}@{}}",
    ]
    for key, value in meta:
        parts.append(
            r"\textbf{\color{accent}" + inline(key) + r"} & " + inline(value) + r" \\[4pt]"
        )
    parts.append(r"\end{tabular}\par")

    if history:
        # El \par anterior es necesario: sin él, LaTeX descarta este \vspace
        # y el bloque queda pegado a la tabla de metadatos.
        parts += [
            r"\vspace{2.4cm}",
            r"\begin{minipage}{0.78\textwidth}",
            r"\small\color{muted}",
            r"\textbf{\color{accent}Historial}\par\vspace{5pt}",
        ]
        for key, value in history:
            parts.append(
                r"\textbf{" + inline(key) + ": }" + inline(value) + r"\par\vspace{3pt}"
            )
        parts.append(r"\end{minipage}")

    parts += [r"\vfill", r"\end{titlepage}"]
    return "\n".join(parts)


# --------------------------------------------------------------------------
# Preámbulo LaTeX
# --------------------------------------------------------------------------

PREAMBLE = r"""
\documentclass[10pt,a4paper]{article}

\usepackage{fontspec}
% es-noshorthands: babel-spanish vuelve activos «"», «~» y «<>». Eso rompe
% cualquier span de código que los contenga, así que se desactivan.
\usepackage[spanish,es-noquoting,es-tabla,es-noshorthands]{babel}
\usepackage[a4paper,top=2.4cm,bottom=2.3cm,left=2.2cm,right=2.2cm,headsep=14pt,footskip=26pt]{geometry}
\usepackage{array}
\usepackage{longtable}
\usepackage{booktabs}
\usepackage[table,dvipsnames]{xcolor}
\usepackage{fancyhdr}
\usepackage{fvextra}  % fancyvrb + breaklines
\usepackage{needspace}
\usepackage{titlesec}
\usepackage{enumitem}
\usepackage{lastpage}
\usepackage{microtype}
\usepackage[hidelinks]{hyperref}

% El título se inyecta desde el script; se usa en el encabezado y en los
% metadatos del PDF, así que debe quedar definido antes de ambos.
\newcommand{\DOCTITLE}{@@DOCTITLE@@}

% ---------------------------------------------------------------- tipografía
\setmainfont{Helvetica Neue}[
  BoldFont   = * Bold,
  ItalicFont = * Italic,
  Ligatures  = TeX,
]
\setmonofont{Menlo}[Scale=0.84]

% Helvetica Neue no trae flechas ni varios símbolos matemáticos. Sin este
% respaldo, «bodega→vitrina» se imprime como una caja vacía.
\usepackage{newunicodechar}
\newfontfamily\symbolfont{Apple Symbols}[Scale=MatchLowercase]
\newunicodechar{→}{{\symbolfont →}}
\newunicodechar{←}{{\symbolfont ←}}
\newunicodechar{↔}{{\symbolfont ↔}}
\newunicodechar{↕}{{\symbolfont ↕}}
\newunicodechar{≥}{{\symbolfont ≥}}
\newunicodechar{≤}{{\symbolfont ≤}}
\newunicodechar{≠}{{\symbolfont ≠}}

% ------------------------------------------------------------------- colores
\definecolor{ink}{HTML}{16202B}
\definecolor{accent}{HTML}{1F5F8B}
\definecolor{muted}{HTML}{667487}
\definecolor{rule}{HTML}{C9D4DF}
\definecolor{tablealt}{HTML}{F4F7FA}
\definecolor{headbg}{HTML}{E6EDF4}
\definecolor{codebg}{HTML}{F5F7F9}
\definecolor{prioM}{HTML}{B3261E}
\definecolor{prioS}{HTML}{9A6400}
\definecolor{prioC}{HTML}{4A6572}
\definecolor{prioW}{HTML}{8A8F98}

\color{ink}

% -------------------------------------------------------------------- títulos
\titleformat{\section}
  {\needspace{4\baselineskip}\normalfont\fontsize{19}{23}\bfseries\color{accent}}
  {\thesection}{0.7em}{}
\titleformat{\subsection}
  {\normalfont\fontsize{13.5}{17}\bfseries\color{ink}}
  {\thesubsection}{0.6em}{}
\titlespacing*{\section}{0pt}{0pt}{14pt}
\titlespacing*{\subsection}{0pt}{16pt}{8pt}

\setlength{\parindent}{0pt}
\setlength{\parskip}{6pt}
\linespread{1.10}

\setlist[itemize]{leftmargin=1.2em,itemsep=3pt,topsep=4pt,parsep=0pt}

% --------------------------------------------------------------- encabezados
\pagestyle{fancy}
\fancyhf{}
\renewcommand{\headrulewidth}{0.4pt}
\renewcommand{\footrulewidth}{0pt}
\renewcommand{\headrule}{\color{rule}\hrule height 0.4pt}
\fancyhead[L]{\small\color{muted}\DOCTITLE}
\fancyhead[R]{\small\color{muted}\nouppercase{\leftmark}}
\fancyfoot[C]{\small\color{muted}\thepage}
\fancypagestyle{plain}{\fancyhf{}\renewcommand{\headrulewidth}{0pt}%
  \fancyfoot[C]{\small\color{muted}\thepage}}

% -------------------------------------------------------------------- tablas
\newcolumntype{L}[1]{>{\raggedright\arraybackslash}p{#1}}
\newcolumntype{C}[1]{>{\centering\arraybackslash}p{#1}}
\newcommand{\tablefont}{\fontsize{8.6}{11.2}\selectfont}
\newcommand{\thead}[1]{\textbf{\color{accent}#1}}
\newcommand{\prio}[2]{{\bfseries\color{#1}#2}}
\renewcommand{\arraystretch}{1.18}
\setlength{\tabcolsep}{5pt}
\setlength{\LTpre}{8pt}
\setlength{\LTpost}{10pt}
\arrayrulecolor{rule}

% --------------------------------------------------------------------- otros
\newcommand{\codeinline}[1]{{\ttfamily\small #1}}
% 6 líneas de reserva: lo que sigue a un subtítulo suele ser una tabla o un
% bloque de código, y dejar el título solo al pie de la página se lee mal.
\newcommand{\blocktitle}[1]{\needspace{6\baselineskip}\vspace{4pt}%
  {\bfseries\color{accent}#1}\par\vspace{-2pt}}
\newcommand{\coverrule}{\textcolor{rule}{\rule{0.72\textwidth}{0.8pt}}}

\DefineVerbatimEnvironment{codeblock}{Verbatim}{
  fontsize=\small, xleftmargin=10pt, xrightmargin=6pt,
  framesep=8pt, frame=leftline, rulecolor=\color{rule},
  baselinestretch=1.05, breaklines=true,
}

\hypersetup{
  pdftitle={\DOCTITLE},
  pdfsubject={Documentación del proyecto Kilo12},
  colorlinks=true, linkcolor=accent, urlcolor=accent,
  bookmarksnumbered=true,
}

\setcounter{tocdepth}{2}
\setcounter{secnumdepth}{2}
"""


def build_tex(md_text: str) -> str:
    lines = md_text.replace("\t", "    ").split("\n")

    # Título (H1) y bloque de metadatos hasta el primer separador.
    title = "Documento"
    start = 0
    for idx, line in enumerate(lines):
        if line.startswith("# "):
            title = line[2:].strip()
            start = idx + 1
            break

    meta_lines: list[str] = []
    body_start = start
    for idx in range(start, len(lines)):
        if lines[idx].strip() in ("---", "***", "___"):
            body_start = idx + 1
            break
        meta_lines.append(lines[idx])

    doc = [
        PREAMBLE.replace("@@DOCTITLE@@", inline(title)),
        r"\begin{document}",
        build_cover(title, meta_lines),
        r"\clearpage",
        r"\pagenumbering{roman}",
        r"\renewcommand{\contentsname}{Contenido}",
        r"\tableofcontents",
        r"\clearpage",
        r"\pagenumbering{arabic}",
        convert_body(lines[body_start:]),
        r"\end{document}",
    ]
    return "\n".join(doc)


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 2

    src = Path(sys.argv[1]).resolve()
    if not src.exists():
        print(f"No existe el archivo: {src}")
        return 1

    if not shutil.which("xelatex"):
        print("Falta xelatex. Instalar MacTeX o TeXLive.")
        return 1

    out_pdf = src.with_suffix(".pdf")
    tex = build_tex(src.read_text(encoding="utf-8"))

    with tempfile.TemporaryDirectory() as tmp:
        tmpdir = Path(tmp)
        tex_file = tmpdir / f"{src.stem}.tex"
        tex_file.write_text(tex, encoding="utf-8")

        # Dos pasadas: la primera resuelve el índice y el total de páginas.
        for run in (1, 2):
            proc = subprocess.run(
                ["xelatex", "-interaction=nonstopmode", "-halt-on-error",
                 tex_file.name],
                cwd=tmpdir, capture_output=True, text=True,
            )
            if proc.returncode != 0:
                log = (tmpdir / f"{src.stem}.log")
                tail = log.read_text(encoding="utf-8", errors="replace")[-4000:] \
                    if log.exists() else proc.stdout[-4000:]
                print(f"xelatex falló en la pasada {run}:\n{tail}")
                # Se conserva el .tex para poder depurarlo.
                shutil.copy(tex_file, Path.cwd() / f"{src.stem}.debug.tex")
                return 1

        log_text = (tmpdir / f"{src.stem}.log").read_text(
            encoding="utf-8", errors="replace"
        )
        shutil.copy(tmpdir / f"{src.stem}.pdf", out_pdf)

    # Un glifo ausente se imprime como caja vacía sin detener la compilación:
    # hay que reportarlo o pasa inadvertido hasta que alguien lea el PDF.
    missing = sorted(set(re.findall(r"Missing character: There is no (\S+)", log_text)))
    if missing:
        print(f"AVISO: glifos ausentes en la fuente: {' '.join(missing)}")

    overfull = len(re.findall(r"Overfull \\hbox \((\d+(?:\.\d+)?)pt", log_text))
    bad = [m for m in re.findall(r"Overfull \\hbox \((\d+(?:\.\d+)?)pt", log_text)
           if float(m) > 10]
    if bad:
        print(f"AVISO: {len(bad)} desbordes de más de 10pt (de {overfull} en total)")

    size_kb = out_pdf.stat().st_size / 1024
    print(f"PDF generado: {out_pdf}  ({size_kb:.0f} KB)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
