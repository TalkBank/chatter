//! Main-tier snippets retain CHAT spelling through the shared serializer.
#![deny(clippy::wildcard_enum_match_arm)]

use talkbank_model::model::{ChatString, UtteranceContent};

/// Render plain CHAT text. Presentation adapters escape it for their own format;
/// sidecar JSON carries this string unchanged.
pub fn format_content_item(item: &UtteranceContent) -> String {
    ChatString::render(|writer| item.write_chat(writer))
}

#[cfg(test)]
mod tests {
    use super::format_content_item;

    #[test]
    fn reference_content_keeps_delimiters_annotations_and_events() {
        for (source, expected) in [
            (
                include_str!("../../../../../corpus/reference/content/pauses-and-events.cha"),
                vec!["(.)", "(..)", "(...)", "&=yawn", "&*MOT:yeah"],
            ),
            (
                include_str!("../../../../../corpus/reference/annotation/groups-regular.cha"),
                vec![
                    "hey [= foo]",
                    "<what in the world> [= bar]",
                    "<the big> [/]",
                    "<<happy and> [?]> [>]",
                ],
            ),
        ] {
            let file = crate::test_fixtures::parse_chat(source);
            let snippets: Vec<_> = file
                .utterances()
                .flat_map(|u| u.main.content.content.iter().map(format_content_item))
                .collect();
            for snippet in expected {
                assert!(
                    snippets.iter().any(|actual| actual == snippet),
                    "missing {snippet}: {snippets:?}"
                );
            }
        }
    }
}
