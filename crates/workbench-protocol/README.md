# Workbench protocol

This crate owns the typed requests, responses, validation, and framed transport shared by native Workbench clients and the Server. It defines contracts without opening sockets or executing operations; runtime owners interpret admitted requests.

## Native plugin runtime contract

Closed plugin invocation/log/get/cancel requests and replies use matched-cohort OBWP kinds 45/46, capped at 32 KiB and 8 MiB respectively. Context carries display data only; admitted Server resolves executable intent and Worker owns processes. Log cursors pin Worker identity/generation and descending sequence; uncertain mutation delivery is explicit. Rust, TypeScript and JSON share the plugin conformance corpus. [Native plugin behavior](../../apps/orchbee/docs/windows-plugins.md) owns runtime lifecycle and remaining integration.
