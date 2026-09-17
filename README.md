# pretty

Wadler-style pretty printing, for [Meadow](https://github.com/mcdearman/meadow).
You build a document from text, line breaks, groups and nesting. Rendering
lays it out in a given width: each group goes on one line if it fits, and is
broken if it does not.

This package is a port of Rust's [`pretty`](https://github.com/Marwes/pretty.rs)
0.12.5. It renders documents exactly as the crate does, annotations included.
Text that is not ASCII is measured with
[unicodeWidth](https://github.com/mcdearman/meadow-unicode-width), the port of
the `unicode-width` version the crate uses.

## Install

```sh
meadow add mcdearman/meadow-pretty
```

## Use

```meadow
use pretty
use Std.Collections.Vector as V

data Sexp = Atom String | List [Sexp]

fun sexp s =
  match s with
  | Sexp.Atom a -> text a
  | Sexp.List xs -> append (text "(") (append (group (nest 1 (intersperse (V.map sexp xs) line))) (text ")"))

def square =
  Sexp.List
    [ Sexp.Atom "define",
      Sexp.List [Sexp.Atom "square", Sexp.Atom "x"],
      Sexp.List [Sexp.Atom "*", Sexp.Atom "x", Sexp.Atom "x"] ]

def main = V.map (\w -> render w (sexp square)) [40, 20, 10]
```

The three widths give:

```text
(define (square x) (* x x))
```

```text
(define
 (square x)
 (* x x))
```

```text
(define
 (square
  x)
 (* x x))
```

The crate builds documents with methods on an allocator. Here they are plain
functions, taking the document last so that calls chain with `|>`. Meadow has
no user-defined operators beyond `++`, so `append` stands in for the crate's
`+`.

### Building

- **Text:**
  - `text s`, where text that is not ASCII is measured in terminal columns;
  - `asString x`;
  - `space`;
  - `reflow s`, which fills lines with the words of `s`.
- **Breaks:**
  - `hardline`, which always breaks;
  - `line`, a break or a space;
  - `line_`, a break or nothing;
  - `softline` and `softline_`, which break only where they must.
- **Combining:** `append a b`, `concat docs`, `intersperse docs sep` and
  `optional (Maybe doc)`.
- **Layout:**
  - `group d`, which is flat if it fits;
  - `nest n d`, `align d`, `hang n d` and `indent n d`;
  - `flatAlt broken flat`.
- **Enclosing:** `enclose before after d`, and `parens`, `brackets`, `braces`,
  `angles`, `singleQuotes` and `doubleQuotes`.
- **Context:** `column f` and `nesting f` build a document from the current
  column or indentation. `widthOf f d` follows `d` with `f` of the width `d`
  took. The crate calls this `width`, which here names the text-measuring
  function.
- **Choice:** `union a b` renders `a` if it can and `b` otherwise. `fail` is a
  document that cannot be rendered.
- **Annotations:** `annotate value d`.
- **Blocks:** `format indent (block [affixes prefix suffix, …] body)` lays a
  body out inside layers of prefixes and suffixes, fitting as many layers on
  the first line as it can. `nested` indents what a layer encloses. This is
  the crate's `BlockDoc`.

The `Doc` constructors are public, as the crate's are. The crate's `Nil` is
`Empty` here, because lists already use `Nil`. A `Text` made by hand is
measured in bytes, as in the crate.

### Rendering

- `render width d` gives `Ok` of the text, or `Err` if the document failed.
- `renderChunks width d` gives the text as `Write` chunks, with `Push` and
  `Pop` around annotated parts, so a caller can style them. Adjacent text is
  joined into one chunk.
- `printDoc width d` prints the text.

## How it's made

The modules in `src/` translate the crate's builders and its renderer.
The renderer's stack of commands is kept entry for entry as the crate keeps
it: a hard line break takes its indentation from the next command on the
stack, and an annotation ends when the stack returns to the size it had. Both
behaviours depend on the stack's exact shape.

**`src/Cases.mw`** holds 2,000 random documents. Each records the calls that
build it and what the crate renders for it, including the annotations, in
three widths. The documents use every builder and every kind of text, and
they include:

- column, nesting and width functions;
- unions and failures;
- blocks.

Run `scripts/generate.sh` to regenerate; it needs a Rust toolchain.

## Licence

MIT, like pretty: see [LICENSE](LICENSE) and [COPYRIGHT](COPYRIGHT).
