//! Typed body entry points selected from retained model labels, not source parsing.

use super::{Invocation, check_payload};
use talkbank_model::{ChatParser, ErrorCollector};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum TypedApi {
    Mor,
    Gra,
    Pho,
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
}

impl TypedApi {
    pub(super) const ALL: [Self; 14] = [
        Self::Mor,
        Self::Gra,
        Self::Pho,
        Self::Sin,
        Self::Act,
        Self::Cod,
        Self::Com,
        Self::Exp,
        Self::Add,
        Self::Gpx,
        Self::Int,
        Self::Spa,
        Self::Sit,
        Self::Wor,
    ];

    pub(super) fn for_label(label: &str) -> Option<Self> {
        Some(match label {
            "mor" => Self::Mor,
            "gra" => Self::Gra,
            "pho" | "mod" => Self::Pho,
            "sin" => Self::Sin,
            "act" => Self::Act,
            "cod" => Self::Cod,
            "com" => Self::Com,
            "exp" => Self::Exp,
            "add" => Self::Add,
            "gpx" => Self::Gpx,
            "int" => Self::Int,
            "spa" => Self::Spa,
            "sit" => Self::Sit,
            "wor" => Self::Wor,
            _ => return None,
        })
    }

    pub(super) fn measure(
        self,
        parser: &impl ChatParser,
        input: &str,
        origin: usize,
        invocation: &Invocation,
        errors: &ErrorCollector,
    ) -> Result<(), String> {
        macro_rules! invoke {
            ($implicit:ident, $explicit:ident) => {{
                let outcome = match invocation {
                    Invocation::Implicit => parser.$implicit(input, origin, errors),
                    Invocation::DocumentContext(context) => {
                        parser.$explicit(input, origin, context, errors)
                    }
                };
                let _ = check_payload(outcome, input.len(), origin, errors)?;
            }};
        }
        match self {
            Self::Mor => invoke!(parse_mor_tier, parse_mor_tier_with_context),
            Self::Gra => invoke!(parse_gra_tier, parse_gra_tier_with_context),
            Self::Pho => invoke!(parse_pho_tier, parse_pho_tier_with_context),
            Self::Sin => invoke!(parse_sin_tier, parse_sin_tier_with_context),
            Self::Act => invoke!(parse_act_tier, parse_act_tier_with_context),
            Self::Cod => invoke!(parse_cod_tier, parse_cod_tier_with_context),
            Self::Com => invoke!(parse_com_tier, parse_com_tier_with_context),
            Self::Exp => invoke!(parse_exp_tier, parse_exp_tier_with_context),
            Self::Add => invoke!(parse_add_tier, parse_add_tier_with_context),
            Self::Gpx => invoke!(parse_gpx_tier, parse_gpx_tier_with_context),
            Self::Int => invoke!(parse_int_tier, parse_int_tier_with_context),
            Self::Spa => invoke!(parse_spa_tier, parse_spa_tier_with_context),
            Self::Sit => invoke!(parse_sit_tier, parse_sit_tier_with_context),
            Self::Wor => invoke!(parse_wor_tier, parse_wor_tier_with_context),
        }
        Ok(())
    }
}
