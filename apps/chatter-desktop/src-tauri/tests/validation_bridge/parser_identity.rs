//! Desktop requests must select the same parser identity for execution and cache.

use super::*;
use chatter_desktop_lib::events::FrontendFileStatus;
use talkbank_transform::ParserKind;

#[test]
fn parser_toggle_reuses_only_its_own_cached_verdict() {
    let cache_dir = tempfile::tempdir().unwrap();
    let state = ValidationState::new_at(cache_dir.path().to_path_buf());
    let file = workspace_root().join("corpus/reference/core/basic-conversation.cha");
    for (parser_kind, expected_hit) in [
        (ParserKind::Re2c, false),
        (ParserKind::TreeSitter, false),
        (ParserKind::Re2c, true),
        (ParserKind::TreeSitter, true),
    ] {
        let config = ValidationConfig {
            parser_kind,
            jobs: Some(1),
            ..ValidationConfig::default()
        };
        let cache = state.cache_for_config(&config);
        let (events, _cancel) =
            validate_target_streaming_with_config(file.clone(), config, cache).unwrap();
        let statuses: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                FrontendEvent::FileComplete { status, .. } => Some(status),
                _ => None,
            })
            .collect();
        assert_eq!(statuses.len(), 1);
        assert!(
            matches!(statuses[0], FrontendFileStatus::Valid { cache_hit } if cache_hit == expected_hit),
            "unexpected cache behavior for {parser_kind:?}: {:?}",
            statuses[0]
        );
    }
}
