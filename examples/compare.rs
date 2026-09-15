//! Score a drawing you made, with the objective this library uses on its own.
//!
//! ```text
//! cargo run --features mermaid --example compare -- graph.mmd other.json
//! ```
//!
//! The drawing can be one you laid out by hand, or the output of a renderer
//! you are writing. It is read from JSON as boxes and polylines in cell
//! coordinates, scored with [`score_drawing`](orthodag::score_drawing), and
//! painted with the same glyphs this library paints its own layout with. The
//! library then lays the graph out itself, and both frames are printed with a
//! table of every metric side by side, so the only thing that differs between
//! the two pictures is the layout.
//!
//! The JSON is
//!
//! ```text
//! { "boxes":  [{"node": 0, "column": 0, "x": 0, "y": 0, "w": 5, "h": 3}, ...],
//!   "routes": [{"edge": 0, "points": [[5, 1], [14, 1]]}, ...] }
//! ```
//!
//! where `node` and `edge` are indices in the order the graph declares them,
//! which is the order the mermaid reader hands them out in. `width` and
//! `height` at the top level are optional; without them the frame is as big as
//! the geometry needs. The first graph in the mermaid file is the one compared.

use orthodag::{Drawing, Graph, Options, Rect, Score};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(graph_path), Some(other_path)) = (args.next(), args.next()) else {
        eprintln!("usage: compare <graph.mmd> <other.json>");
        return;
    };

    let Ok(source) = std::fs::read_to_string(&graph_path) else {
        eprintln!("cannot read {graph_path}");
        return;
    };
    let Ok(mut read) = orthodag::io::mermaid_in::from_mermaid(&source) else {
        eprintln!("that is not a flowchart this can read");
        return;
    };
    if read.is_empty() {
        eprintln!("no graph in {graph_path}");
        return;
    }
    let (title, g) = read.swap_remove(0);

    let Ok(json) = std::fs::read_to_string(&other_path) else {
        eprintln!("cannot read {other_path}");
        return;
    };
    let drawing = match Parser::new(&json)
        .document()
        .and_then(|v| to_drawing(&g, &v))
    {
        Ok(drawing) => drawing,
        Err(why) => {
            eprintln!("{other_path}: {why}");
            return;
        }
    };

    let options = Options::default();
    let theirs = orthodag::score_drawing(&g, &drawing);
    let ours = orthodag::score_with(&g, options);

    if !title.is_empty() {
        println!("──── {title} ────");
    }
    println!("════ supplied ════");
    print!("{}", orthodag::draw_drawing(&g, &drawing, options));
    println!("\n════ this library ════");
    print!("{}", orthodag::draw_with(&g, options));
    println!();
    table(&theirs, &ours);
}

/// Every metric, theirs beside ours, and the difference.
fn table(theirs: &Score, ours: &Score) {
    println!(
        "{:<10} {:>8} {:>8} {:>8}",
        "metric", "supplied", "library", "delta"
    );
    for ((name, a), (_, b)) in theirs.fields().into_iter().zip(ours.fields()) {
        println!("{name:<10} {a:>8} {b:>8} {:>+8}", b - a);
    }
}

/// The supplied geometry as a `Drawing` over this graph's ids.
fn to_drawing(g: &Graph, json: &Json) -> Result<Drawing, String> {
    let nodes: Vec<_> = g.node_ids().collect();
    let edges: Vec<_> = g.edge_ids().collect();

    let boxes = json.list("boxes")?;
    let routes = json.list("routes")?;

    // The frame is what the caller says, or else what the geometry reaches.
    let mut width = 0;
    let mut height = 0;
    for b in &boxes {
        width = width.max(b.int("x")? + b.int("w")?);
        height = height.max(b.int("y")? + b.int("h")?);
    }
    for r in &routes {
        for (x, y) in points(r)? {
            width = width.max(x + 1);
            height = height.max(y + 1);
        }
    }
    if let Ok(w) = json.int("width") {
        width = w;
    }
    if let Ok(h) = json.int("height") {
        height = h;
    }

    let mut drawing = Drawing::new(width, height);
    for b in &boxes {
        let at = index(b.int("node")?)?;
        let node = nodes
            .get(at)
            .ok_or_else(|| format!("node {at} is not in the graph"))?;
        let column = index(b.int("column")?)?;
        drawing.boxed(
            *node,
            column,
            Rect::new(b.int("x")?, b.int("y")?, b.int("w")?, b.int("h")?),
        );
    }
    for r in &routes {
        let at = index(r.int("edge")?)?;
        let edge = edges
            .get(at)
            .ok_or_else(|| format!("edge {at} is not in the graph"))?;
        drawing.route(*edge, points(r)?);
    }
    Ok(drawing)
}

fn index(n: i32) -> Result<usize, String> {
    usize::try_from(n).map_err(|_| format!("{n} is not an index"))
}

fn points(route: &Json) -> Result<Vec<(i32, i32)>, String> {
    route
        .list("points")?
        .iter()
        .map(|p| match p {
            Json::Array(xy) if xy.len() == 2 => Ok((xy[0].int_value()?, xy[1].int_value()?)),
            _ => Err("a point is [x, y]".to_owned()),
        })
        .collect()
}

/// As much JSON as the drawing needs: numbers, arrays and objects. Anything
/// else is read past and kept as nothing.
#[derive(Debug)]
enum Json {
    Number(i64),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
    Other,
}

impl Json {
    fn field(&self, name: &str) -> Result<&Json, String> {
        match self {
            Json::Object(fields) => fields
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .ok_or_else(|| format!("no \"{name}\"")),
            _ => Err(format!(
                "\"{name}\" wanted on something that is not an object"
            )),
        }
    }

    fn int(&self, name: &str) -> Result<i32, String> {
        self.field(name)?.int_value()
    }

    fn int_value(&self) -> Result<i32, String> {
        match self {
            Json::Number(n) => i32::try_from(*n).map_err(|_| format!("{n} does not fit a cell")),
            _ => Err("expected a number".to_owned()),
        }
    }

    fn list(&self, name: &str) -> Result<Vec<&Json>, String> {
        match self.field(name)? {
            Json::Array(items) => Ok(items.iter().collect()),
            _ => Err(format!("\"{name}\" is not an array")),
        }
    }
}

/// A recursive-descent reader over the bytes of the document.
struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
            at: 0,
        }
    }

    fn document(&mut self) -> Result<Json, String> {
        let value = self.value()?;
        self.skip_space();
        if self.at == self.bytes.len() {
            Ok(value)
        } else {
            Err(self.fail("trailing characters"))
        }
    }

    fn fail(&self, what: &str) -> String {
        format!("{what} at byte {}", self.at)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn eat(&mut self, want: u8) -> Result<(), String> {
        self.skip_space();
        if self.peek() == Some(want) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.fail(&format!("expected '{}'", char::from(want))))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_space();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => self.string().map(|_| Json::Other),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => self.literal(),
            None => Err(self.fail("unexpected end")),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.eat(b'{')?;
        let mut fields = Vec::new();
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(Json::Object(fields));
        }
        loop {
            self.skip_space();
            let key = self.string()?;
            self.eat(b':')?;
            let value = self.value()?;
            fields.push((key, value));
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(Json::Object(fields));
                }
                _ => return Err(self.fail("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.eat(b'[')?;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(b']') {
            self.at += 1;
            return Ok(Json::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                _ => return Err(self.fail("expected ',' or ']'")),
            }
        }
    }

    /// A string with no escapes beyond `\"` and `\\`, which is all a key needs.
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return String::from_utf8(out).map_err(|_| self.fail("bad utf-8"));
                }
                Some(b'\\') => {
                    self.at += 1;
                    match self.peek() {
                        Some(c @ (b'"' | b'\\' | b'/')) => out.push(c),
                        Some(b'n') => out.push(b'\n'),
                        Some(b't') => out.push(b'\t'),
                        _ => return Err(self.fail("unsupported escape")),
                    }
                    self.at += 1;
                }
                Some(c) => {
                    out.push(c);
                    self.at += 1;
                }
                None => return Err(self.fail("unterminated string")),
            }
        }
    }

    /// Integers only. A cell coordinate has no fraction.
    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        if self.peek() == Some(b'-') {
            self.at += 1;
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or("");
        text.parse()
            .map(Json::Number)
            .map_err(|_| self.fail("expected an integer"))
    }

    /// `true`, `false`, `null`: read past and ignored.
    fn literal(&mut self) -> Result<Json, String> {
        for word in ["true", "false", "null"] {
            if self.bytes[self.at..].starts_with(word.as_bytes()) {
                self.at += word.len();
                return Ok(Json::Other);
            }
        }
        Err(self.fail("unexpected character"))
    }
}
