//! Finite API-policy carriers, derived from the existing fragment decks and
//! the authored options/shortening rules. These are public API tests, not a
//! second CHAT grammar or an exhaustive specification census.

#[derive(Clone, Copy, Debug)]
pub(super) enum Api {
    Header,
    Id,
    Participant,
    Utterance,
    Main,
    Word,
    MorTier,
    MorWord,
    GraTier,
    GraRelation,
    PhoTier,
    PhoWord,
    Sin,
    Act,
    Cod,
    Com,
    Exp,
    Add,
    Gpx,
    Int,
    Spa,
    Sit,
    Wor,
    Dependent,
}

/// Clean source and one structural mutation at each supported fragment route.
pub(super) const BASE: &[(Api, &str, &str)] = &[
    (
        Api::Header,
        "@Languages:\teng, spa\n",
        "@Languages:\teng,, spa\n",
    ),
    (
        Api::Id,
        "@ID:\teng|corpus|CHI|||||Child|||\n",
        "@ID:\teng|corpus|CHI",
    ),
    (Api::Participant, "CHI Target_Child", "CHI"),
    (Api::Utterance, "*CHI:\tcafé .\n", "*CHI:\t<café\n"),
    (Api::Main, "*CHI:\tcafé .\n", "*CHI:\t<café\n"),
    (Api::Word, "café", "café@"),
    (Api::MorTier, "n|café .", "n|café"),
    (Api::MorWord, "n|café", "|café"),
    (Api::GraTier, "1|0|ROOT", "1|"),
    (Api::GraRelation, "1|0|ROOT", "1|"),
    (Api::PhoTier, "kːæt", "k:æt"),
    (Api::PhoWord, "kːæt", "k:æt"),
    (Api::Sin, "g:point", "g:point \u{15}"),
    (Api::Act, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Cod, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Com, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Exp, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Add, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Gpx, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Int, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Spa, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Sit, "café \u{15}1_2\u{15}", "café \u{15}"),
    (Api::Wor, "café &-uh .", "café & ."),
    (
        Api::Dependent,
        "%com:\tcafé \u{15}1_2\u{15}\n",
        "%com:\tcafé \u{15}\n",
    ),
];

/// The options rule changes parenthetical meaning, not CA-symbol admissibility.
/// Each carrier also supplies a whole-shortening refusal without CA context.
pub(super) const OMISSIONS: &[(Api, &str)] = &[
    (Api::Main, "*CHI:\t(café) .\n"),
    (Api::Main, "*CHI:\t⌊(café) .\n"),
    (Api::Main, "*CHI:\t<(café)> [//] café .\n"),
    (Api::Main, "*CHI:\tword [: (café)] .\n"),
    (Api::Utterance, "*CHI:\t(café) .\n"),
    (
        Api::Utterance,
        "*CHI:\t<(café)> [//] café .\n%com:\tretained\n",
    ),
];

/// Main-tier notation that remains legal without the CA option. Parenthetical
/// shortening with lexical material is deliberately not an omission.
pub(super) const NOTATION: &[(Api, &str)] = &[
    (Api::Main, "*CHI:\t(be)cause .\n"),
    (Api::Main, "*CHI:\t0(café) .\n"),
    (Api::Main, "*CHI:\tcafé ↗\n"),
    (Api::Utterance, "*CHI:\t(be)cause .\n%com:\tretained\n"),
    (Api::Header, "@Languages:\teng,\n\tspa\n"),
];
