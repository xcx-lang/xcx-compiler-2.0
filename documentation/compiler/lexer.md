# XCX Lexer (Scanner)

The Lexer is responsible for converting the raw source character stream into a stream of discrete tokens.

## Implementation Details

- **File**: `src/lexer/scanner.rs`
- **Technique**: Manual, eager, character-by-character scanning.
- **API**: Single method `next_token(&mut self, interner: &mut Interner) -> Token`, called on demand by the parser (not an iterator).
- **Lookahead**: Single-character lookahead via `peek()` and two-character via `peek_next()` / `peek_at(offset)`.

## Token Types

Tokens are defined in `src/lexer/token.rs` as the `TokenKind` enum. Each `Token` carries a `Span` (line, col, len) for error reporting.

Key categories:

| Category | Examples |
|---|---|
| Literals | `IntLiteral(i64)`, `FloatLiteral(f64)`, `StringLiteral(StringId)`, `True`, `False` |
| Type keywords | `TypeI`, `TypeF`, `TypeS`, `TypeB`, `Array`, `Set`, `Map`, `Table`, `Json`, `Date`, `Fiber` |
| Set type keywords | `TypeSetN`, `TypeSetQ`, `TypeSetZ`, `TypeSetS`, `TypeSetB`, `TypeSetC` |
| Control flow | `If`, `Then`, `ElseIf`, `Else`, `End`, `While`, `Do`, `For`, `In`, `To`, `Break`, `Continue` |
| Functions/Fibers | `Func`, `Return`, `Fiber`, `Yield` |
| Operators | `Plus`, `Minus`, `Star`, `Slash`, `Caret`, `PlusPlus`, `Has`, `And`, `Or`, `Not` |
| Set operators | `Union`, `Intersection`, `Difference`, `SymDifference` |
| Special punctuation | `GreaterBang` (`>!`), `GreaterQuestion` (`>?`), `DoubleColon` (`::`), `DoubleComma` (`,,`), `Bridge` (`<->`) |
| Builtins | `Net`, `Serve`, `Store`, `Halt`, `Terminal`, `Json`, `Date` |
| Special | `RawBlock(StringId)`, `AtStep`, `AtAuto`, `AtWait` |

## Special Scanning Features

### Raw Blocks
Delimited by `<<<` and `>>>`. Everything between is captured as a single `RawBlock(StringId)` token, used for inline JSON or multi-line string data.

```
<<<
  { "key": "value" }
>>>
```

### Comments
XCX uses `---` as comment delimiter:
- **Single-line**: `--- this is a comment` (content on same line after `---`)
- **Multi-line**: `---` followed by only whitespace until end of line opens a block, closed by `*---`

### Unicode Set Operators
The scanner recognises Unicode symbols directly as tokens:
- `∪` → `TokenKind::Union`
- `∩` → `TokenKind::Intersection`
- `\` → `TokenKind::Difference`
- `⊕` → `TokenKind::SymDifference`

### Else/ElseIf Disambiguation
The scanner peeks ahead after recognising `else` / `els` to check if the next word is `if` — if so, it collapses the two words into a single `ElseIf` token.

### `@` Directives
Tokens starting with `@` are scanned as named directives:
- `@step` → `AtStep`
- `@auto` → `AtAuto`
- `@wait` → `AtWait`

## String Allocation

All identifiers and string literals are passed through `Interner::intern()`, returning a `StringId (u32)`. The raw `String` is stored once in the interner; the rest of the pipeline works with numeric IDs. There is no zero-copy optimisation — strings are always heap-allocated into the interner's internal `Vec<String>`.