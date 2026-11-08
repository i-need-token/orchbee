//! The Rust registry mirror against `schemas/connection/v1/connections.json`, in both directions,
//! and the compiled fingerprints against this checkout.

use std::{collections::BTreeMap, path::Path};

use orchbee_connection_handshake::{ConnectionKind, Delivery, FrameFormat, GenerationRange};
use serde::Deserialize;

use crate::{algorithm, support::TestResult};

const REGISTRY: &str = include_str!("../../../schemas/connection/v1/connections.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Registry {
    negotiated_connections: BTreeMap<String, NegotiatedConnection>,
    frame_formats: BTreeMap<String, Contract>,
    deliveries: BTreeMap<String, Contract>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct NegotiatedConnection {
    generations: GenerationRange,
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    generations: GenerationRange,
}

#[test]
fn rust_constants_mirror_the_registry() -> TestResult {
    let registry: Registry = serde_json::from_str(REGISTRY)?;

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
        let capabilities = connection.capabilities();
        let mirrored: Vec<&str> = capabilities.iter().map(|name| name.as_str()).collect();
        assert_eq!(names, mirrored, "{} capabilities", connection.name());
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
    let registry: Registry = serde_json::from_str(REGISTRY)?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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

fn sorted(mut names: Vec<&str>) -> Vec<&str> {
    names.sort_unstable();
    names
}
