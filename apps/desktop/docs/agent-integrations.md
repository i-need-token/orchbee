# Agent Integrations

Desktop Settings and the CLI enable and disable the
[observation adapters](../resources/agent-observation-adapters/README.md) as jobs of the Session's
Worker. The Worker's [integration writer](../../../crates/agent-integration-writer/README.md) owns
the exact managed entries, conflict handling, and rollback, and its provider conformance cases pin
them. The [Agent integration contract](../../../crates/agent-integration/README.md) owns the values
and report vocabularies.

This page remains only because the adapter Capsule's README links it, and editing that README
changes the Capsule's payload digest. The Capsule's move out of `apps/desktop`, planned in the
[native runtime topology proposal](../../../proposals/platform/native-runtime-topology.md#single-agent-integration-writer),
rewrites that README once and removes this page.
