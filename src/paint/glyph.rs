//! Which character a set of direction bits is.
//!
//! A pure function of the bits and nothing else: eleven box-drawing characters
//! plus a space. Nothing here knows which edge left which bit, how many runs
//! passed through, or in what order they were painted — and that is the point.
//! A fork and a crossing arrive here as the same four bits and leave as the same
//! `┼`, because to a reader they are the same mark.

use super::grid::{D, L, R, U};

/// The mask of the four bits that mean anything.
const ALL: u8 = L | R | U | D;

/// The character for a cell.
pub(crate) fn glyph(bits: u8) -> char {
    match bits & ALL {
        0 => ' ',
        b if b == L || b == R || b == L | R => '─',
        b if b == U || b == D || b == U | D => '│',
        b if b == R | D => '┌',
        b if b == L | D => '┐',
        b if b == R | U => '└',
        b if b == L | U => '┘',
        b if b == L | R | D => '┬',
        b if b == L | R | U => '┴',
        b if b == U | D | R => '├',
        b if b == U | D | L => '┤',
        _ => '┼',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_is_a_space() {
        assert_eq!(glyph(0), ' ');
    }

    #[test]
    fn a_run_and_its_ends_are_the_same_character() {
        // The end of a run has one bit and its middle has two; both are line.
        assert_eq!(glyph(L), '─');
        assert_eq!(glyph(R), '─');
        assert_eq!(glyph(L | R), '─');
        assert_eq!(glyph(U), '│');
        assert_eq!(glyph(D), '│');
        assert_eq!(glyph(U | D), '│');
    }

    #[test]
    fn two_bits_at_a_right_angle_are_a_corner() {
        assert_eq!(glyph(R | D), '┌');
        assert_eq!(glyph(L | D), '┐');
        assert_eq!(glyph(R | U), '└');
        assert_eq!(glyph(L | U), '┘');
    }

    #[test]
    fn three_bits_are_a_tee_pointing_away_from_the_missing_one() {
        assert_eq!(glyph(L | R | D), '┬');
        assert_eq!(glyph(L | R | U), '┴');
        assert_eq!(glyph(U | D | R), '├');
        assert_eq!(glyph(U | D | L), '┤');
    }

    #[test]
    fn four_bits_are_a_cross() {
        assert_eq!(glyph(L | R | U | D), '┼');
    }

    #[test]
    fn bits_outside_the_four_are_ignored() {
        for spare in [16u8, 32, 64, 128] {
            assert_eq!(glyph(L | R | spare), '─');
            assert_eq!(glyph(spare), ' ');
        }
    }

    #[test]
    fn every_combination_has_a_character() {
        for bits in 0..=ALL {
            let drawn = glyph(bits);
            assert_eq!(
                drawn == ' ',
                bits == 0,
                "{bits:04b} drew {drawn:?}, which is the wrong kind of nothing"
            );
        }
    }
}
