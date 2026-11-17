//! The Rust registry mirror against `schemas/connection/v1/connections.json`, in both directions,
//! and the compiled fingerprints against this checkout. The registry's shape lives here for the
//! compatibility checks too.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use orchbee_connection_handshake::{
    ConnectionKind, Delivery, FrameFormat, Generation, GenerationRange, ProductVersion,
};
use serde::Deserialize;

use crate::{algorithm, support::TestResult};

const REGISTRY: &str = include_str!("../../../schemas/connection/v1/connections.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Registry {
    negotiated_connections: BTreeMap<String, NegotiatedConnection>,
    frame_formats: BTreeMap<String, Contract>,
    deliveries: BTreeMap<String, Contract>,
    pub frozen_fixtures: Vec<FrozenFixture>,
}

impl Registry {
    pub fn load() -> serde_json::Result<Self> {
        serde_json::from_str(REGISTRY)
    }

    /// Every connection, frame format, and delivery: its name, supported generations, and
    /// superseded generations.
    pub fn contracts(&self) -> impl Iterator<Item = (&str, GenerationRange, &[Supersession])> {
        let connections = self.negotiated_connections.iter().map(|(name, entry)| {
            (
                name.as_str(),
                entry.generations,
                entry.superseded.as_slice(),
            )
        });
        let others = self.frame_formats.iter().chain(&self.deliveries);
        connections.chain(others.map(|(name, entry)| {
            (
                name.as_str(),
                entry.generations,
                entry.superseded.as_slice(),
            )
        }))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct NegotiatedConnection {
    generations: GenerationRange,
    superseded: Vec<Supersession>,
    capabilities: Vec<Capability>,
    fingerprint_inputs: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Capability {
    name: String,
    #[expect(
        dead_code,
        reason = "the release that added a name is history, not behavior"
    )]
    added_in: String,
}

/// A frame format or delivery entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Contract {
    pub generations: GenerationRange,
    pub superseded: Vec<Supersession>,
}

/// A generation that a released later generation superseded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Supersession {
    pub generation: Generation,
    pub superseded_in: ProductVersion,
    pub superseded_on: String,
}

/// A fixture file that freezes once a release ships it; `sha256` seals it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenFixture {
    pub path: String,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[test]
fn rust_constants_mirror_the_registry() -> TestResult {
    let registry = Registry::load()?;

    let declared: Vec<&str> = registry
        .negotiated_connections
        .keys()
        .map(String::as_str)
        .collect();
    let mirrored: Vec<&str> = ConnectionKind::ALL.iter().map(|kind| kind.name()).collect();
    assert_eq!(declared, sorted(mirrored));
    for connection in ConnectionKind::ALL {
        let entry = registry
            .negotiated_connections
            .get(connection.name())
            .ok_or("every connection is declared")?;
        assert_eq!(connection.supported_generations(), entry.generations);
        let names: Vec<&str> = entry
            .capabilities
            .iter()
            .map(|capability| capability.name.as_str())
            .collect();
        let declared_capabilities = connection.declared_capabilities();
        let mirrored: Vec<&str> = declared_capabilities
            .iter()
            .map(|name| name.as_str())
            .collect();
        assert_eq!(names, mirrored, "{} capabilities", connection.name());
        // A build offers only names the registry declares.
        for name in connection.capabilities().iter() {
            assert!(
                mirrored.contains(&name.as_str()),
                "{} offers {}",
                connection.name(),
                name.as_str()
            );
        }
    }

    let declared: Vec<&str> = registry.frame_formats.keys().map(String::as_str).collect();
    assert_eq!(
        declared,
        sorted(
            FrameFormat::ALL
                .iter()
                .map(|format| format.name())
                .collect()
        )
    );
    for format in FrameFormat::ALL {
        let entry = registry
            .frame_formats
            .get(format.name())
            .ok_or("every format is declared")?;
        assert_eq!(format.supported_generations(), entry.generations);
    }

    let declared: Vec<&str> = registry.deliveries.keys().map(String::as_str).collect();
    assert_eq!(
        declared,
        sorted(
            Delivery::ALL
                .iter()
                .map(|delivery| delivery.name())
                .collect()
        )
    );
    for delivery in Delivery::ALL {
        let entry = registry
            .deliveries
            .get(delivery.name())
            .ok_or("every delivery is declared")?;
        assert_eq!(delivery.supported_generations(), entry.generations);
    }
    Ok(())
}

#[test]
fn compiled_fingerprints_describe_this_checkout() -> TestResult {
    let registry = Registry::load()?;
    let repository = repository();
    for connection in ConnectionKind::ALL {
        let inputs = &registry
            .negotiated_connections
            .get(connection.name())
            .ok_or("every connection is declared")?
            .fingerprint_inputs;
        let inputs: Vec<&str> = inputs.iter().map(String::as_str).collect();
        assert_eq!(
            &algorithm::compute(&repository, &inputs)?,
            connection.development_fingerprint().as_bytes(),
            "{} fingerprint",
            connection.name()
        );
    }
    Ok(())
}

/// The repository root of this checkout.
pub fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sorted(mut names: Vec<&str>) -> Vec<&str> {
    names.sort_unstable();
    names
}
