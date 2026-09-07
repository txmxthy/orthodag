# Painting edges

The painter converts boxes and orthogonal polylines into a grid of Unicode
characters. Edge segments contribute direction bits to each cell, and a
separate overlay holds borders, text and arrowheads.

## Direction bits

Each cell holds four bits. A set bit indicates that a line extends from the
cell in that direction.

```text
L = 1    R = 2    U = 4    D = 8
```

A horizontal segment sets `R` on every cell except its right endpoint and `L`
on every cell except its left endpoint. Vertical segments use `D` and `U` in
the same way. Each endpoint therefore has a bit pointing into the segment,
which combines with any segment that meets it. A zero-length segment sets no
bits.

Contributions are combined with bitwise OR. Once all routes have been walked,
the accumulated bits determine the glyph.

## Glyph table

| Bits | Glyph |
|---|---|
| none | space |
| `L`, `R`, `L R` | `─` |
| `U`, `D`, `U D` | `│` |
| `R D` | `┌` |
| `L D` | `┐` |
| `R U` | `└` |
| `L U` | `┘` |
| `L R D` | `┬` |
| `L R U` | `┴` |
| `U D R` | `├` |
| `U D L` | `┤` |
| `L R U D` | `┼` |

Corners result from horizontal and vertical segments meeting in a cell. The
painter does not need separate corner operations, and routes can contribute
their bits in any order. Tests cover both properties.

The bits alone do not distinguish a connected junction from unrelated edges
crossing. Both can produce `┼`. Crossing classification supplies the information
needed to draw bridges.

## Overlay

Box borders, labels, captions and arrowheads are written into an overlay that
takes precedence over the edge grid. Keeping borders separate prevents their
bits from combining with nearby edges into unintended junctions. Boxes also
cover any routes passing underneath them.

The painter skips diagonal segments. It does not approximate them with an
orthogonal path, so a routing error can leave a visible gap.

## Arrowheads

A forward edge arriving from the left ends with `▶`. A back edge rising from
its lane ends with `▲`. The final segment determines the direction.

## Crossings and bridges

A bridge replaces the crossing cell with `│` and shortens the horizontal line
on either side:

```text
───┼───        becomes        ──╴│╶──
```

Only neighbouring cells containing plain horizontal line are shortened. Corners
and junctions beside the crossing remain intact.

The scorer supplies the crossing cells so that scoring and painting use the
same definition. This avoids drawing a bridge at a cell that the objective
classifies as an overlap.

`Crossing::Bridge` applies bridges at every crossing, while `Crossing::Cross`
keeps the combined direction glyphs. The default, `Crossing::ByColour`, uses
bridges between distinct flows. It keeps a junction when the edges share both
an ink and an endpoint. Matching tags alone are insufficient, and untagged
edges are treated as distinct inks for this classification.

## Colour and spans

Each edge cell carries a palette slot. Contributions with the same slot retain
it; conflicting slots mark the cell as mixed and clear its slot. The caller
then renders that cell in its default colour.

This records the conflict without arbitrarily assigning the cell to one flow.
Layout tries to prevent such conflicts by giving different tag sets separate
attach rows. Genuine crossings can still require two colours in one cell,
which the output cannot represent.

`spans` returns each row as runs with a shared colour and drawing part. A caller
can apply one terminal style, HTML element or widget style per run. Palette
slots identify flows; the caller decides how those slots look.
