//! Domain-indexed measuring-group policy shared by immutable and mutable walks.
#![deny(clippy::wildcard_enum_match_arm)]

use std::convert::Infallible;

use super::domain::TierDomain;

/// The payload is selected at the producer, before any consumer sees it.
pub(super) enum Measured<A> {
    Enter,
    Atomic(A),
    Excluded,
}

pub(super) enum MeasuringGroup<P, S> {
    Pho(P),
    Sin(S),
}

/// One table, usable with borrowed groups or with unit payloads for mutable
/// traversal. No cast from an erased atomic group is needed.
pub(super) trait Measurement {
    type Atomic<P, S>;
    fn measure<P, S>(group: MeasuringGroup<P, S>) -> Measured<Self::Atomic<P, S>>;
}

pub(super) struct Unmeasured;
pub(super) struct Phonological;
pub(super) struct Signed;

impl Measurement for Unmeasured {
    type Atomic<P, S> = Infallible;

    fn measure<P, S>(_group: MeasuringGroup<P, S>) -> Measured<Infallible> {
        Measured::Enter
    }
}

impl Measurement for Phonological {
    type Atomic<P, S> = P;

    fn measure<P, S>(group: MeasuringGroup<P, S>) -> Measured<P> {
        match group {
            MeasuringGroup::Pho(group) => Measured::Atomic(group),
            MeasuringGroup::Sin(_) => Measured::Excluded,
        }
    }
}

impl Measurement for Signed {
    type Atomic<P, S> = S;

    fn measure<P, S>(group: MeasuringGroup<P, S>) -> Measured<S> {
        match group {
            MeasuringGroup::Pho(_) => Measured::Excluded,
            MeasuringGroup::Sin(group) => Measured::Atomic(group),
        }
    }
}

/// Static domain identity travels with every position the shared walk emits.
pub(super) trait TraversalDomain {
    type Measurement: Measurement;
    const DOMAIN: Option<TierDomain>;
}

#[derive(Clone, Copy)]
pub(super) struct Mor;
#[derive(Clone, Copy)]
pub(super) struct Pho;
#[derive(Clone, Copy)]
pub(super) struct Sin;
pub(super) struct Wor;
pub(super) struct Unscoped;

impl TraversalDomain for Mor {
    type Measurement = Unmeasured;
    const DOMAIN: Option<TierDomain> = Some(TierDomain::Mor);
}

impl TraversalDomain for Pho {
    type Measurement = Phonological;
    const DOMAIN: Option<TierDomain> = Some(TierDomain::Pho);
}

impl TraversalDomain for Sin {
    type Measurement = Signed;
    const DOMAIN: Option<TierDomain> = Some(TierDomain::Sin);
}

impl TraversalDomain for Wor {
    type Measurement = Unmeasured;
    const DOMAIN: Option<TierDomain> = Some(TierDomain::Wor);
}

impl TraversalDomain for Unscoped {
    type Measurement = Unmeasured;
    const DOMAIN: Option<TierDomain> = None;
}

pub(super) type AtomicFor<'a, D> = <<D as TraversalDomain>::Measurement as Measurement>::Atomic<
    &'a crate::model::PhoGroup,
    &'a crate::model::SinGroup,
>;
