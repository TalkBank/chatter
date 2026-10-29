//! Compiled macro-consumer boundaries, not canonical CHAT coverage.

mod caller_macro {
    use crate::model::WriteChat;

    macro_rules! write {
        ($sink:expr, $($tokens:tt)*) => {
            $sink.write_str("caller macro")
        };
    }

    crate::string_newtype! { struct Plain; }
    crate::interned_newtype! {
        struct Interned,
        interner: crate::model::stem_interner()
    }

    #[test]
    fn caller_macro_cannot_replace_wrapper_display() -> std::fmt::Result {
        use std::fmt::Write;
        let mut control = String::new();
        write!(&mut control, "control")?;
        assert_eq!(control, "caller macro");
        let plain = Plain::new("word");
        let interned = Interned::new("word");
        assert_eq!(plain.as_str(), "word");
        assert_eq!(interned.as_str(), "word");
        assert_eq!(plain.to_string(), "word");
        assert_eq!(interned.to_string(), "word");
        assert_eq!(plain.to_chat_string(), "word");
        assert_eq!(interned.to_chat_string(), "word");
        Ok(())
    }
}

mod caller_types {
    use crate::model::WriteChat;

    crate::string_newtype! { struct String; }
    crate::interned_newtype! {
        struct Result,
        interner: crate::model::stem_interner()
    }
    struct Vec(::std::vec::Vec<u32>);
    crate::collection_newtype_ops!(Vec, u32);

    #[test]
    fn wrapper_names_do_not_replace_runtime_types() -> serde_json::Result<()> {
        let text = String::new("word");
        assert_eq!(text.as_str(), "word");
        assert_eq!(String::from(::std::string::String::from("word")), text);
        assert_eq!(text.to_chat_string(), "word");
        assert_eq!(
            serde_json::from_str::<String>(&serde_json::to_string(&text)?)?,
            text
        );

        let interned = Result::new("word");
        assert_eq!(interned.as_str(), "word");
        assert_eq!(Result::from(::std::string::String::from("word")), interned);
        assert_eq!(interned.to_chat_string(), "word");
        assert_eq!(
            serde_json::from_str::<Result>(&serde_json::to_string(&interned)?)?,
            interned
        );

        let mut items = Vec(vec![1, 2, 3]);
        assert_eq!(items.as_slice(), &[1, 2, 3]);
        items.as_mut_slice()[1] = 4;
        items.retain(|item| *item != 1);
        assert_eq!(items.take(), vec![4, 3]);
        assert!(items.into_vec().is_empty());
        Ok(())
    }
}
