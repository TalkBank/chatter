//! Deterministic alphabetic placeholders, without numeric CHAT word content
//! or a fixed-width counter that can wrap during document traversal.

use talkbank_model::model::NonEmptyString;
use talkbank_model::non_empty_literal;

/// One digit of the alphabetic ordinal. Only these letters can enter a suffix.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
enum Letter {
    A = b'a',
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
}

enum LetterStep {
    Next(Letter),
    Carry,
}

impl Letter {
    fn successor(self) -> LetterStep {
        use Letter::*;
        LetterStep::Next(match self {
            A => B,
            B => C,
            C => D,
            D => E,
            E => F,
            F => G,
            G => H,
            H => I,
            I => J,
            J => K,
            K => L,
            L => M,
            M => N,
            N => O,
            O => P,
            P => Q,
            Q => R,
            R => S,
            S => T,
            T => U,
            U => V,
            V => W,
            W => X,
            X => Y,
            Y => Z,
            Z => return LetterStep::Carry,
        })
    }

    fn character(self) -> char {
        char::from(self as u8)
    }
}

/// A nonempty alphabetic ordinal: `a` through `z`, then `aa`, `ab`, ….
///
/// Construction starts at [`Self::first`]; [`Self::successor`] consumes an
/// ordinal and advances it without a numeric limit or an invalid zero index.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlaceholderIndex {
    // Least significant first. A separate first letter makes emptiness
    // unrepresentable, including before the first traversal item.
    first: Letter,
    rest: Vec<Letter>,
}

impl PlaceholderIndex {
    /// The first ordinal, `a`.
    pub fn first() -> Self {
        Self {
            first: Letter::A,
            rest: Vec::new(),
        }
    }

    /// Consume this ordinal and produce the following one.
    pub fn successor(mut self) -> Self {
        self.advance();
        self
    }

    fn advance(&mut self) {
        for letter in std::iter::once(&mut self.first).chain(&mut self.rest) {
            match letter.successor() {
                LetterStep::Next(next) => {
                    *letter = next;
                    return;
                }
                LetterStep::Carry => *letter = Letter::A,
            }
        }
        self.rest.push(Letter::A);
    }

    fn suffix(&self) -> String {
        self.rest
            .iter()
            .rev()
            .chain(std::iter::once(&self.first))
            .map(|letter| letter.character())
            .collect()
    }
}

/// Alphabetic text used in serialized CHAT output. Construction combines a
/// fixed prefix with an alphabetic ordinal; numeric digits and empty text
/// cannot be emitted. The word prefix cannot produce `xxx`, `yyy` or `www`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaceholderToken(NonEmptyString);

impl PlaceholderToken {
    /// Builds a word placeholder (`worda`, `wordb`, …).
    pub fn word(index: PlaceholderIndex) -> Self {
        Self(non_empty_literal!("word").with_suffix(&index.suffix()))
    }

    /// Builds a MOR lemma placeholder (`lemmaa`, `lemmab`, …).
    ///
    /// Word and lemma replacements share one sequence in document order;
    /// lemma suffixes need not match corresponding main-tier word suffixes.
    pub fn lemma(index: PlaceholderIndex) -> Self {
        Self(non_empty_literal!("lemma").with_suffix(&index.suffix()))
    }

    /// Borrows the placeholder text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// The placeholder as proven nonempty text.
    pub fn text(&self) -> &NonEmptyString {
        &self.0
    }
}

impl AsRef<str> for PlaceholderToken {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

/// Per-document sequence. There is always an admitted next ordinal.
#[derive(Debug)]
pub struct PlaceholderState {
    next: PlaceholderIndex,
}

impl PlaceholderState {
    /// Creates a fresh sequence starting at `a`.
    pub fn new() -> Self {
        Self {
            next: PlaceholderIndex::first(),
        }
    }

    /// Returns the next ordinal and advances the document's sequence.
    pub fn next(&mut self) -> PlaceholderIndex {
        let current = self.next.clone();
        self.next.advance();
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabetic_carries_keep_distinct_ordinals() {
        let mut state = PlaceholderState::new();
        let mut index = PlaceholderIndex::first();
        let mut seen = std::collections::HashSet::new();
        for ordinal in 1..=703 {
            assert_eq!(state.next(), index);
            let suffix = index.suffix();
            assert!(seen.insert(suffix.clone()));
            match ordinal {
                1 => assert_eq!(suffix, "a"),
                26 => assert_eq!(suffix, "z"),
                27 => assert_eq!(suffix, "aa"),
                52 => assert_eq!(suffix, "az"),
                53 => assert_eq!(suffix, "ba"),
                702 => assert_eq!(suffix, "zz"),
                703 => assert_eq!(suffix, "aaa"),
                _ => {}
            }
            index = index.successor();
        }
    }

    #[test]
    fn word_and_lemma_prefixes_do_not_emit_untranscribed_markers() {
        let mut index = PlaceholderIndex::first();
        // Includes suffix ww, which would form the special token www with
        // the former one-letter prefix. These are output policy boundaries.
        for _ in 0..703 {
            let word = PlaceholderToken::word(index.clone());
            let lemma = PlaceholderToken::lemma(index.clone());
            for token in [&word, &lemma] {
                assert!(token.as_str().bytes().all(|byte| byte.is_ascii_lowercase()));
                assert!(!["xxx", "yyy", "www"].contains(&token.as_str()));
            }
            assert_ne!(word, lemma);
            index = index.successor();
        }
    }
}
