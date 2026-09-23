//! Terminal-cell measurement and preparation for text supplied by callers.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// How many terminal cells `text` occupies.
pub(crate) fn width(text: &str) -> usize {
    UnicodeWidthStr::width(sanitise(text).as_ref())
}

/// Replaces characters that can control a terminal or its bidi state.
pub(crate) fn sanitise(text: &str) -> Cow<'_, str> {
    if !text.chars().any(unsafe_scalar) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(
        text.chars()
            .map(|ch| if unsafe_scalar(ch) { '\u{fffd}' } else { ch })
            .collect(),
    )
}

/// Prepares `text` to occupy at most `room` terminal cells.
pub(crate) fn fit(text: &str, room: usize) -> String {
    let text = sanitise(text);
    if UnicodeWidthStr::width(text.as_ref()) <= room {
        return text.into_owned();
    }
    if room == 0 {
        return String::new();
    }

    let mut used = 0;
    text.graphemes(true)
        .take_while(|grapheme| {
            let next = used + UnicodeWidthStr::width(*grapheme);
            if next > room - 1 {
                return false;
            }
            used = next;
            true
        })
        .chain(std::iter::once("…"))
        .collect()
}

/// Visits sanitised grapheme clusters with the terminal cells each occupies.
pub(crate) fn graphemes(text: &str, mut visit: impl FnMut(&str, usize)) {
    let text = sanitise(text);
    for grapheme in text.graphemes(true) {
        visit(grapheme, UnicodeWidthStr::width(grapheme));
    }
}

fn unsafe_scalar(ch: char) -> bool {
    matches!(
        ch,
        '\u{0}'..='\u{1f}' | '\u{7f}'..='\u{9f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}
