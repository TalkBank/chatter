//! Public header adapters exercised with already-parsed reference/spec values.
//! These are output and wire contracts, not extra CHAT parsing or validity claims.

use std::path::Path;
use talkbank_model::model::{ChatFile, Header, SesValue, WriteChat};

/// The four SES variants remain distinct in the witness inventory, especially
/// unsupported source text versus an absent optional ID field.
#[derive(Default)]
pub(super) struct HeaderWitnesses {
    headers: usize,
    ethnicity: usize,
    socioeconomic: usize,
    combined: usize,
    unsupported: usize,
}

impl HeaderWitnesses {
    pub(super) fn observe(&mut self, file: &ChatFile, path: &Path) {
        for header in file.headers() {
            let mut output = String::new();
            header
                .write_chat(&mut output)
                .expect("accepting header sink");
            assert_eq!(header.to_chat(), output);
            assert_eq!(header.to_string(), output);
            super::assert_display_refusals(header, path);
            self.headers += 1;

            let Header::ID(id) = header else { continue };
            let Some(ses) = &id.ses else { continue };
            match ses {
                SesValue::EthOnly(_) => self.ethnicity += 1,
                SesValue::SesOnly(_) => self.socioeconomic += 1,
                SesValue::Combined { .. } => self.combined += 1,
                SesValue::Unsupported(_) => self.unsupported += 1,
            }
            // Explicit public lexical construction, not reparsing serialized CHAT.
            // The parsed value supplies the admitted vocabulary or preserved text.
            let spelling = ses.as_str();
            assert_eq!(SesValue::from(spelling.as_str()), *ses);
            assert_eq!(SesValue::from(spelling.clone()), *ses);
            assert_eq!(ses.to_string(), spelling);
            let mut output = String::new();
            ses.write_chat(&mut output).expect("accepting SES sink");
            assert_eq!(output, spelling);
            assert!(super::assert_writer_refusals(ses, path) > 0);
            assert!(super::assert_display_refusals(ses, path) > 0);
            let wire = serde_json::to_value(ses).expect("SES wire value");
            assert_eq!(wire, serde_json::Value::String(spelling));
            assert_eq!(
                serde_json::from_value::<SesValue>(wire).expect("SES wire admission"),
                *ses
            );
        }
    }

    pub(super) fn assert_reference_witnesses(&self) {
        assert!(self.headers > 0);
        assert!(
            self.socioeconomic > 0 && self.combined > 0,
            "reference headers must supply standalone and combined SES values"
        );
    }

    pub(super) fn assert_spec_witnesses(&self) {
        assert!(self.headers > 0);
        assert!(
            self.ethnicity > 0 && self.combined > 0 && self.unsupported > 0,
            "spec headers must supply legal ethnicity/combined and unsupported SES values"
        );
        // External wire shape rejection is distinct from a preserved unsupported
        // string. No invalid typed model is fabricated to exercise this boundary.
        assert!(serde_json::from_value::<SesValue>(serde_json::Value::Bool(false)).is_err());
    }
}
