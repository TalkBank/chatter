//! Independent token/source admission controls for marker-chain leaves.

use super::*;

#[test]
fn zero_action_admission_keeps_source_and_refuses_foreign_or_nonzero_tokens() {
    let input = String::from("prefix 0");
    let source = SourceText::new(&input);
    let tokens = [Token::Zero(&input[7..])];
    let parsed = zero_action(source)
        .parse(tokens.as_slice())
        .into_result()
        .unwrap();
    let ContentItem::Action { zero, .. } = &parsed else {
        panic!("expected action");
    };
    assert_eq!(zero.span(), talkbank_model::Span::new(7, 8));
    assert_eq!(zero.lexeme(), "0");
    assert_eq!(
        serde_json::to_value(&parsed).unwrap()["Action"]["zero"],
        "0"
    );
    let foreign = input.clone();
    assert!(
        zero_action(SourceText::new(&foreign))
            .parse(tokens.as_slice())
            .into_result()
            .is_err()
    );
    for text in ["", "00", "word"] {
        let tokens = [Token::Zero(text)];
        assert!(
            zero_action(SourceText::new(text))
                .parse(tokens.as_slice())
                .into_result()
                .is_err()
        );
    }
    let at_zero = ZeroAction::new(SourceText::new("0").whole_slice()).unwrap();
    assert_eq!(*zero, at_zero, "action equality retains spelling");
}

#[test]
fn event_admission_keeps_extent_descriptor_and_wire_without_mutable_model_location() {
    let input = String::from("prefix &=laughs");
    let source = SourceText::new(&input);
    let tokens = [Token::Event(crate::token::Extracted::from_lexed(
        &input[9..],
        &input[7..],
    ))];
    let parsed = event(source)
        .parse(tokens.as_slice())
        .into_result()
        .unwrap();
    let ContentItem::Event(event, _) = &parsed else {
        panic!("expected event");
    };
    assert_eq!(event.span(), talkbank_model::Span::new(7, input.len()));
    assert_eq!(event.event_type().as_str(), "laughs");
    let model = event.to_model();
    assert_eq!(model.location.span(), Some(event.span()));
    assert_eq!(
        serde_json::to_value(event).unwrap(),
        serde_json::to_value(&model).unwrap()
    );
    let mut detached_model = model;
    detached_model.location = talkbank_model::Location::Unlocated;
    assert_eq!(event.to_model().location.span(), Some(event.span()));
    let foreign = input.clone();
    assert!(
        super::event(SourceText::new(&foreign))
            .parse(tokens.as_slice())
            .into_result()
            .is_err()
    );
    let invalid = [Token::Event(crate::token::Extracted::from_lexed(
        "",
        &input[7..],
    ))];
    assert!(
        super::event(source)
            .parse(invalid.as_slice())
            .into_result()
            .is_err()
    );
}
