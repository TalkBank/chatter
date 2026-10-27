//! Authored constructs exercise peer parsers through their declared public APIs.
//! This is parse-only semantic comparison, not CST identity or file validation.

mod observation;
mod policy;
mod terminators;
use generators::{ConstructSpec, repo_paths::RepoRoot, templates::TemplateLoader};
use observation::compare_parses;
use policy::{Invocation, PolicyPair};
use std::collections::BTreeSet;
use talkbank_model::ChatParser;

type Failure = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Copy, Debug)]
enum Api {
    File,
    Header,
    MainTier,
    Utterance,
    Word,
    DependentTier,
    WrappedOverlap,
}

/// File parsing has no contextual overload. Only fragment calls carry a policy.
#[derive(Clone, Copy, Debug)]
enum Call {
    File,
    Fragment(FragmentApi, PolicyPair),
}

#[derive(Clone, Copy, Debug)]
enum FragmentApi {
    Header,
    MainTier,
    Utterance,
    Word,
    DependentTier,
}

#[derive(Clone, Copy, Debug)]
enum Placement {
    Start,
    Embedded,
}

impl Placement {
    fn offset(self) -> usize {
        match self {
            Self::Start => 0,
            Self::Embedded => "@Comment:\tPréface café\n".len(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Transport {
    Authored,
    CrLf,
    Cr,
}

impl Transport {
    fn apply(self, input: &str) -> String {
        let ending = match self {
            Self::Authored => return input.to_owned(),
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        };
        input
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .replace('\n', ending)
    }
}

impl Api {
    fn admit(level: &str, fence: &str) -> Result<Self, Failure> {
        // A Markdown `chat` fence is not itself a file-level contract. The
        // authored Overlap Points example declares a main-tier shape.
        if fence == "chat" {
            return match level {
                "main_tier" => Ok(Self::MainTier),
                "file" => Ok(Self::File),
                unknown => Err(format!("unmapped chat-fence construct level: {unknown}").into()),
            };
        }
        match fence {
            "document" => Ok(Self::File),
            "main_tier" => Ok(Self::MainTier),
            "utterance" => Ok(Self::Utterance),
            "standalone_word" => Ok(Self::Word),
            "overlap_point" => Ok(Self::WrappedOverlap),
            "activities_header"
            | "comment_header"
            | "date_header"
            | "id_header"
            | "languages_header"
            | "location_header"
            | "media_header"
            | "options_header"
            | "participants_header"
            | "pid_header"
            | "situation_header"
            | "time_duration_header"
            | "time_start_header"
            | "transcriber_header"
            | "types_header" => Ok(Self::Header),
            "com_dependent_tier" | "gra_dependent_tier" | "mor_dependent_tier"
            | "pho_dependent_tier" | "wor_dependent_tier" => Ok(Self::DependentTier),
            unknown => Err(format!("unmapped authored construct fence: {unknown}").into()),
        }
    }

    fn fragment(self) -> Option<FragmentApi> {
        Some(match self {
            Self::File => return None,
            Self::Header => FragmentApi::Header,
            Self::MainTier | Self::WrappedOverlap => FragmentApi::MainTier,
            Self::Utterance => FragmentApi::Utterance,
            Self::Word => FragmentApi::Word,
            Self::DependentTier => FragmentApi::DependentTier,
        })
    }

    fn calls(self) -> Vec<Call> {
        let Some(fragment) = self.fragment() else {
            return vec![Call::File];
        };
        Invocation::ALL
            .into_iter()
            .map(|invocation| Call::Fragment(fragment, PolicyPair::Same(invocation)))
            .collect()
    }
}

impl Call {
    fn compare(
        self,
        primary: &impl ChatParser,
        native: &impl ChatParser,
        input: &str,
        placement: Placement,
    ) -> Result<(), String> {
        let offset = placement.offset();
        let (api, invocation) = match self {
            Self::File => {
                return compare_parses(
                    |sink| primary.parse_chat_file(input, offset, sink),
                    |sink| native.parse_chat_file(input, offset, sink),
                );
            }
            Self::Fragment(api, invocation) => (api, invocation),
        };
        let (left, right) = invocation.invocations();
        let left_context = left.context();
        let right_context = right.context();
        macro_rules! pair {
            ($ordinary:ident, $explicit:ident) => {
                compare_parses(
                    |sink| match &left_context {
                        None => primary.$ordinary(input, offset, sink),
                        Some(context) => primary.$explicit(input, offset, context, sink),
                    },
                    |sink| match &right_context {
                        None => native.$ordinary(input, offset, sink),
                        Some(context) => native.$explicit(input, offset, context, sink),
                    },
                )
            };
        }
        match api {
            FragmentApi::Header => pair!(parse_header, parse_header_with_context),
            FragmentApi::MainTier => pair!(parse_main_tier, parse_main_tier_with_context),
            FragmentApi::Utterance => pair!(parse_utterance, parse_utterance_with_context),
            FragmentApi::Word => pair!(parse_word, parse_word_with_context),
            FragmentApi::DependentTier => {
                pair!(parse_dependent_tier, parse_dependent_tier_with_context)
            }
        }
    }
}

/// Only nonempty categories with uniquely identified examples reach parsing.
struct ConstructDeck(Vec<Case>);
struct Case {
    label: String,
    api: Api,
    input: String,
}
impl ConstructDeck {
    fn load(root: &RepoRoot) -> Result<Self, Failure> {
        Self::admit(
            ConstructSpec::load_all(root.join("spec/constructs"))?,
            &TemplateLoader::new(&root.join("spec/tools/templates"))?,
        )
    }
    fn admit(specs: Vec<ConstructSpec>, templates: &TemplateLoader) -> Result<Self, Failure> {
        let mut labels = BTreeSet::new();
        let mut cases = Vec::new();
        for spec in specs {
            if spec.examples.is_empty() {
                return Err(format!(
                    "empty construct category: {}/{}",
                    spec.metadata.level, spec.metadata.category
                )
                .into());
            }
            for example in spec.examples {
                let label = format!(
                    "construct/{}/{}/{}",
                    spec.metadata.level, spec.metadata.category, example.name
                );
                if !labels.insert(label.clone()) {
                    return Err(format!("duplicate construct identity: {label}").into());
                }
                let api = Api::admit(&spec.metadata.level, &example.input_type)?;
                let input = match api {
                    Api::WrappedOverlap => {
                        templates.wrap_fragment(&example.input_type, &example.input)?
                    }
                    Api::File
                    | Api::Header
                    | Api::MainTier
                    | Api::Utterance
                    | Api::Word
                    | Api::DependentTier => example.input,
                };
                cases.push(Case { label, api, input });
            }
        }
        if cases.is_empty() {
            return Err("empty construct population; no test acceptance".into());
        }
        Ok(Self(cases))
    }
}

#[test]
fn authored_constructs_use_the_corresponding_public_api() -> Result<(), Failure> {
    let root = RepoRoot::resolve(None)?;
    let deck = ConstructDeck::load(&root)?;
    let primary = talkbank_parser::TreeSitterParser::new()?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut observations = 0;
    let mut failures = Vec::new();
    for case in &deck.0 {
        for transport in [Transport::Authored, Transport::CrLf, Transport::Cr] {
            let input = transport.apply(&case.input);
            for placement in [Placement::Start, Placement::Embedded] {
                for call in case.api.calls() {
                    observations += 1;
                    if let Err(failure) = call.compare(&primary, &native, &input, placement) {
                        failures.push(format!(
                            "{} [{call:?} {transport:?} {placement:?}]: {failure}",
                            case.label
                        ));
                    }
                }
            }
        }
    }
    eprintln!(
        "{} authored examples; {observations} comparisons; {} failures",
        deck.0.len(),
        failures.len()
    );
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

#[test]
fn noalign_preserves_each_backends_construct_meaning() -> Result<(), Failure> {
    let deck = ConstructDeck::load(&RepoRoot::resolve(None)?)?;
    let primary = talkbank_parser::TreeSitterParser::new()?;
    let native = talkbank_parser_re2c::Re2cParser::new();
    let mut observations = 0;
    let mut failures = Vec::new();
    for case in &deck.0 {
        // The file variant cannot acquire a fragment context.
        let Some(api) = case.api.fragment() else {
            continue;
        };
        for transport in [Transport::Authored, Transport::CrLf, Transport::Cr] {
            let input = transport.apply(&case.input);
            for placement in [Placement::Start, Placement::Embedded] {
                for policies in [PolicyPair::OrdinaryNoAlign, PolicyPair::CaNoAlign] {
                    let call = Call::Fragment(api, policies);
                    for (backend, result) in [
                        (
                            "tree-sitter",
                            call.compare(&primary, &primary, &input, placement),
                        ),
                        ("re2c", call.compare(&native, &native, &input, placement)),
                    ] {
                        observations += 1;
                        if let Err(failure) = result {
                            failures.push(format!("{} [{backend} {policies:?} {transport:?} {placement:?}]: {failure}", case.label));
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "{observations} NoAlign invariance comparisons; {} failures",
        failures.len()
    );
    if observations == 0 {
        return Err("empty NoAlign comparison population".into());
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

#[test]
fn deck_refuses_empty_duplicate_and_unmapped_populations() -> Result<(), Failure> {
    let root = RepoRoot::resolve(None)?;
    let templates = TemplateLoader::new(&root.join("spec/tools/templates"))?;
    assert!(ConstructDeck::admit(Vec::new(), &templates).is_err());
    let mut specs = ConstructSpec::load_all(root.join("spec/constructs"))?;
    specs[0].examples.clear();
    assert!(
        ConstructDeck::admit(specs, &templates).is_err(),
        "empty category must not disappear inside a nonempty population"
    );
    let mut specs = ConstructSpec::load_all(root.join("spec/constructs"))?;
    let spec = specs
        .iter_mut()
        .find(|spec| !spec.examples.is_empty())
        .ok_or("no example")?;
    spec.examples.push(spec.examples[0].clone());
    let Err(failure) = ConstructDeck::admit(specs, &templates) else {
        panic!("duplicate admitted")
    };
    assert!(failure.to_string().contains("duplicate construct identity"));
    let mut specs = ConstructSpec::load_all(root.join("spec/constructs"))?;
    let spec = specs
        .iter_mut()
        .find(|spec| !spec.examples.is_empty())
        .ok_or("no example")?;
    spec.examples[0].input_type = "unmapped_future_shape".to_owned();
    let Err(failure) = ConstructDeck::admit(specs, &templates) else {
        panic!("unknown shape admitted")
    };
    assert!(
        failure
            .to_string()
            .contains("unmapped authored construct fence")
    );
    assert!(matches!(Api::admit("main_tier", "chat")?, Api::MainTier));
    assert!(Api::admit("word", "chat").is_err());
    Ok(())
}
