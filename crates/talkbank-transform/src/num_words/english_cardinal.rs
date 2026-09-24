//! English cardinal composition with admitted decimal scale units.

use std::collections::BTreeMap;
use std::num::NonZeroU64;

/// A decimal scale whose table phrase explicitly supplies one unit.
/// The divisor cannot be zero, and the unit cannot include the multiplier.
struct Scale<'table> {
    divisor: NonZeroU64,
    unit: &'table str,
}

impl<'table> Scale<'table> {
    fn admit(n: u64, table: &'table BTreeMap<String, String>) -> Option<Self> {
        let mut divisor = NonZeroU64::MIN;
        let thousand = NonZeroU64::new(1000)?;
        while let Some(next) = divisor.checked_mul(thousand) {
            if next.get() > n {
                break;
            }
            divisor = next;
        }
        if divisor == NonZeroU64::MIN {
            return None;
        }
        let one = table.get("1")?;
        let phrase = table.get(&divisor.to_string())?;
        let unit = phrase.strip_prefix(one.as_str())?.strip_prefix(' ')?;
        if unit.is_empty() {
            return None;
        }
        Some(Self { divisor, unit })
    }
}

/// Preserve exact lexical entries; otherwise compose English short-scale groups.
/// Missing lexical data refuses expansion rather than inventing spoken words.
pub(super) fn expand(n: u64, table: &BTreeMap<String, String>) -> Option<String> {
    if let Some(exact) = table.get(&n.to_string()) {
        return Some(exact.clone());
    }
    let (head, remainder) = if n < 1000 {
        let hundreds = n / 100 * 100;
        // Values below one hundred are table-owned, not recursively guessed.
        if hundreds == 0 {
            return None;
        }
        (table.get(&hundreds.to_string())?.clone(), n % 100)
    } else {
        let scale = Scale::admit(n, table)?;
        let multiplier = expand(n / scale.divisor.get(), table)?;
        (
            format!("{multiplier} {}", scale.unit),
            n % scale.divisor.get(),
        )
    };
    if remainder == 0 {
        Some(head)
    } else {
        Some(format!("{head} {}", expand(remainder, table)?))
    }
}
