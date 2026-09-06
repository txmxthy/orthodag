//! Every graph on one page, to be looked at.
//!
//! ```text
//! just gallery
//! ```
//!
//! The scorer puts a number on a drawing. The number is not the drawing. This
//! is the other half of the loop: one static page holding every graph at every
//! option worth comparing, sorted by name or by score, with somewhere to write
//! down what is wrong with each frame.
//!
//! Notes live in the browser's own storage, keyed by graph and tab. That means
//! no server to start and nothing to clean up, at the cost of the notes being
//! stuck to one browser — which is the right trade for a tool one person runs
//! on one machine.

#[path = "../tests/common/mod.rs"]
mod common;

use std::fmt::Write as _;
use std::path::PathBuf;

use orthodag::{Crossing, Graph, Options, Score, Span};

/// The frames each graph is shown in.
fn tabs() -> Vec<(&'static str, Options)> {
    vec![
        ("plain", Options::new()),
        ("labels", Options::new().labels(true)),
        ("bridge", Options::new().crossings(Crossing::Bridge)),
        ("narrow 80", Options::new().width(80)),
    ]
}

fn main() {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/gallery/index.html");
    let wide = std::env::args().any(|arg| arg == "--wide");

    let mut graphs: Vec<(String, Graph)> = common::fixtures()
        .into_iter()
        .map(|(name, g)| (name.to_owned(), g))
        .collect();
    if wide {
        graphs.extend(common::generated(12));
        graphs.extend(common::corpus());
    }

    let page = page(&graphs);
    if let Some(dir) = out.parent()
        && let Err(why) = std::fs::create_dir_all(dir)
    {
        println!("cannot write {}: {why}", dir.display());
        return;
    }
    match std::fs::write(&out, page) {
        Ok(()) => println!("wrote {}", out.display()),
        Err(why) => println!("cannot write {}: {why}", out.display()),
    }
}

fn page(graphs: &[(String, Graph)]) -> String {
    let mut nav = String::new();
    let mut main = String::new();

    for (at, (name, g)) in graphs.iter().enumerate() {
        let score = orthodag::score(g);
        let _ = write!(
            nav,
            "<a data-at=\"{at}\" data-score=\"{}\" data-name=\"{}\">{}<b>{}</b></a>",
            score.total,
            escape(name),
            escape(name),
            score.total
        );
        let _ = write!(main, "{}", section(at, name, g, &score));
    }

    format!(
        "<!doctype html><meta charset=\"utf-8\"><title>orthodag</title>\n\
         <style>{STYLE}</style>\n\
         <nav><h1>orthodag</h1>\
         <p>Every graph, every option worth comparing. Notes are kept in this \
         browser.</p>\
         <button id=\"sort\">sort by score</button>{nav}</nav>\n\
         <main>{main}</main>\n\
         <script>{SCRIPT}</script>\n"
    )
}

fn section(at: usize, name: &str, g: &Graph, score: &Score) -> String {
    let mut tabs_html = String::new();
    let mut frames = String::new();

    for (which, (label, options)) in tabs().into_iter().enumerate() {
        let _ = write!(
            tabs_html,
            "<button data-tab=\"{which}\"{}>{label}</button>",
            if which == 0 { " class=\"on\"" } else { "" }
        );
        let _ = write!(
            frames,
            "<pre data-tab=\"{which}\"{}>{}</pre>",
            if which == 0 { " class=\"on\"" } else { "" },
            frame(&orthodag::spans_with(g, options))
        );
    }

    format!(
        "<section data-at=\"{at}\"><h2>{}<small>{} nodes · {} edges</small></h2>\
         <p class=\"score\">{score}</p><div class=\"tabs\">{tabs_html}</div>{frames}\
         <textarea data-notes=\"{}\" placeholder=\"what is wrong with this one\"></textarea>\
         </section>",
        escape(name),
        g.nodes().len(),
        g.edges().len(),
        escape(name)
    )
}

/// A drawing as HTML, one span per run of colour.
fn frame(rows: &[Vec<Span>]) -> String {
    let mut out = String::new();
    for row in rows {
        for span in row {
            match span.colour {
                Some(slot) => {
                    let _ = write!(
                        out,
                        "<i class=\"c{}\">{}</i>",
                        slot.slot(),
                        escape(&span.text)
                    );
                }
                None => out.push_str(&escape(&span.text)),
            }
        }
        out.push('\n');
    }
    out
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const STYLE: &str = "
:root { color-scheme: dark; --bg:#11121a; --fg:#c8ccd8; --dim:#5b6070; }
* { box-sizing: border-box }
body { margin:0; display:flex; background:var(--bg); color:var(--fg);
       font:13px/1.5 ui-sans-serif, system-ui, sans-serif }
nav { width:270px; flex:none; height:100vh; overflow:auto; padding:16px;
      border-right:1px solid #23252f }
nav h1 { font-size:15px; margin:0 0 4px }
nav p { color:var(--dim); margin:0 0 12px }
nav button { width:100%; margin-bottom:10px; padding:6px; cursor:pointer;
             background:#1b1d27; color:var(--fg); border:1px solid #2b2e3a; border-radius:4px }
nav a { display:flex; justify-content:space-between; gap:8px; padding:4px 6px;
        cursor:pointer; border-radius:4px }
nav a:hover { background:#1b1d27 }
nav a.on { background:#252838 }
nav a b { color:var(--dim); font-weight:400 }
main { flex:1; height:100vh; overflow:auto; padding:20px 24px }
section { display:none }
section.on { display:block }
h2 { font-size:15px; margin:0 0 2px }
h2 small { color:var(--dim); font-weight:400; margin-left:10px }
.score { color:var(--dim); margin:0 0 12px; font:12px ui-monospace, monospace }
.tabs { margin-bottom:10px }
.tabs button { margin-right:6px; padding:3px 9px; cursor:pointer;
               background:#1b1d27; color:var(--dim); border:1px solid #2b2e3a; border-radius:4px }
.tabs button.on { color:var(--fg); border-color:#4a4f66 }
pre { display:none; margin:0 0 14px; padding:12px; background:#0c0d13;
      border:1px solid #23252f; border-radius:6px; overflow:auto;
      font:13px/1.25 ui-monospace, SFMono-Regular, Menlo, monospace }
pre.on { display:block }
textarea { width:100%; height:70px; padding:8px; background:#0c0d13; color:var(--fg);
           border:1px solid #23252f; border-radius:6px; font:inherit; resize:vertical }
i { font-style:normal }
.c0{color:#6cc5d9} .c1{color:#c98fd4} .c2{color:#d9c26c} .c3{color:#7fc98f}
.c4{color:#7f9fd9} .c5{color:#d98080}
";

const SCRIPT: &str = r"
const links = [...document.querySelectorAll('nav a')];
const sections = [...document.querySelectorAll('section')];
const show = at => {
  links.forEach(l => l.classList.toggle('on', l.dataset.at === String(at)));
  sections.forEach(s => s.classList.toggle('on', s.dataset.at === String(at)));
  localStorage.setItem('orthodag:at', at);
};
links.forEach(l => l.onclick = () => show(l.dataset.at));

// One tab choice for the whole page: comparing the same frame across graphs is
// the thing this is for.
const tab = () => localStorage.getItem('orthodag:tab') || '0';
const useTab = which => {
  localStorage.setItem('orthodag:tab', which);
  document.querySelectorAll('[data-tab]').forEach(el =>
    el.classList.toggle('on', el.dataset.tab === String(which)));
};
document.querySelectorAll('.tabs button').forEach(b =>
  b.onclick = () => useTab(b.dataset.tab));

document.querySelectorAll('textarea').forEach(box => {
  const key = 'orthodag:note:' + box.dataset.notes;
  box.value = localStorage.getItem(key) || '';
  box.oninput = () => localStorage.setItem(key, box.value);
});

let byScore = false;
document.getElementById('sort').onclick = e => {
  byScore = !byScore;
  e.target.textContent = byScore ? 'sort by name' : 'sort by score';
  const nav = links[0].parentNode;
  [...links].sort((a, b) => byScore
    ? Number(b.dataset.score) - Number(a.dataset.score)
    : a.dataset.name.localeCompare(b.dataset.name)
  ).forEach(l => nav.appendChild(l));
};

useTab(tab());
show(localStorage.getItem('orthodag:at') || '0');
";
