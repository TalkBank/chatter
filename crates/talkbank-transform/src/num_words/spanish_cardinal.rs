//! Bounded Spanish cardinals; complete table phrases are never scale units.

use std::collections::BTreeMap;

/// A standalone cardinal group in 0..1000.
struct Group(u16);

/// A multiplier that needs no unresolved apocopation before `mil`.
struct Thousands(Group);

enum Cardinal {
    Group(Group),
    Thousands { head: Thousands, tail: Group },
}

#[derive(Debug)]
pub(super) enum UnsupportedCardinal {
    NounScale,
    Agreement,
    MissingLexeme,
}

impl Cardinal {
    fn admit(n: u64) -> Result<Self, UnsupportedCardinal> {
        if n < 1000 {
            return Ok(Self::Group(Group(n as u16)));
        }
        if n >= 1_000_000 {
            return Err(UnsupportedCardinal::NounScale);
        }
        let head = (n / 1000) as u16;
        // `mil` alone needs no multiplier. Other groups ending in uno
        // require an agreement policy that this context-free API cannot supply.
        if head != 1 && head % 10 == 1 && head % 100 != 11 {
            return Err(UnsupportedCardinal::Agreement);
        }
        Ok(Self::Thousands {
            head: Thousands(Group(head)),
            tail: Group((n % 1000) as u16),
        })
    }
}

impl Group {
    fn render(&self, table: &BTreeMap<String, String>) -> Result<String, UnsupportedCardinal> {
        if let Some(exact) = table.get(&self.0.to_string()) {
            return Ok(exact.clone());
        }
        let hundreds = self.0 / 100 * 100;
        let tail = self.0 % 100;
        if hundreds == 0 {
            return Err(UnsupportedCardinal::MissingLexeme);
        }
        let head = if hundreds == 100 && tail != 0 {
            "ciento"
        } else {
            table
                .get(&hundreds.to_string())
                .ok_or(UnsupportedCardinal::MissingLexeme)?
                .as_str()
        };
        if tail == 0 {
            return Ok(head.to_owned());
        }
        let tail = table
            .get(&tail.to_string())
            .ok_or(UnsupportedCardinal::MissingLexeme)?;
        Ok(format!("{head} {tail}"))
    }
}

pub(super) fn expand(
    n: u64,
    table: &BTreeMap<String, String>,
) -> Result<String, UnsupportedCardinal> {
    match Cardinal::admit(n)? {
        Cardinal::Group(group) => group.render(table),
        Cardinal::Thousands {
            head: Thousands(head),
            tail,
        } => {
            let unit = table
                .get("1000")
                .ok_or(UnsupportedCardinal::MissingLexeme)?;
            let prefix = if head.0 == 1 {
                unit.clone()
            } else {
                format!("{} {unit}", head.render(table)?)
            };
            if tail.0 == 0 {
                Ok(prefix)
            } else {
                Ok(format!("{prefix} {}", tail.render(table)?))
            }
        }
    }
}
