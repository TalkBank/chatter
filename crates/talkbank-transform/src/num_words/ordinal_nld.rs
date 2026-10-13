//! Bounded Dutch suffix ordinals, built from the shared cardinal lexicon.
//!
//! Generation joins hundreds directly to the final ordinal component:
//! `honderdeerste`, `honderdtweede`. Variants with `en` may also be correct;
//! this is a deterministic generation policy, not pronunciation inference.

use super::num2text::NUM2LANG;

/// Only whole-token ASCII ranks 1..=1000 can reach Dutch composition.
/// The private field has no raw public constructor or default value.
struct SupportedDutchOrdinal(u16);

impl SupportedDutchOrdinal {
    fn admit(token: &str) -> Option<Self> {
        // Longest suffix first: stripping only `e` would leave `29st`.
        let digits = token
            .strip_suffix("ste")
            .or_else(|| token.strip_suffix("de"))
            .or_else(|| token.strip_suffix('e'))
            .or_else(|| token.strip_suffix('ᵉ'))?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let rank = digits.parse::<u16>().ok()?;
        if !(1..=1000).contains(&rank) {
            return None;
        }
        Some(Self(rank))
    }

    fn render(self) -> Option<String> {
        let rank = self.0;
        match rank {
            1 => Some("eerste".into()),
            3 => Some("derde".into()),
            8 => Some("achtste".into()),
            100..=999 if !rank.is_multiple_of(100) => {
                let head = cardinal(rank / 100 * 100)?;
                // This nonzero remainder is again inside the admitted domain.
                let tail = Self(rank % 100).render()?;
                Some(format!("{head}{tail}"))
            }
            _ => {
                let word = cardinal(rank)?;
                let suffix = if rank < 20 { "de" } else { "ste" };
                Some(format!("{word}{suffix}"))
            }
        }
    }
}

/// Missing lexical evidence refuses expansion rather than inventing a word.
fn cardinal(rank: u16) -> Option<&'static str> {
    NUM2LANG
        .get("nld")?
        .get(&rank.to_string())
        .map(String::as_str)
}

/// Lowercase suffixes are accepted independently of rank (ASR compatibility),
/// including superscript `ᵉ` and leading zeros. Canonical output is written words.
pub(super) fn try_expand_ordinal(token: &str) -> Option<String> {
    SupportedDutchOrdinal::admit(token)?.render()
}
