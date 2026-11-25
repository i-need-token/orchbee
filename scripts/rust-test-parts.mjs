// The test parts of scripts/run-rust-workspace.mjs. CI runs the Rust test task as parts in parallel
// jobs, because one job ran for an hour; together the parts run every target of the unsplit task,
// each once. The terminal client's process tests take most of that time, so its package is three
// parts: its unit tests with its heaviest test target, its longest Windows command targets, and its
// other test targets.

export const clientPackage = 'orchbee';

/** Run with the client's unit tests in `client`. */
export const heavyClientTests = Object.freeze(['integration']);

/**
 * The client's longest Windows command targets, run in `client-long-commands`. Cargo runs one test
 * target after another, and these three took 566 of the 1,180 seconds that the client's command
 * targets ran on `windows-latest` (2026-11-25), so the two command parts finish close together.
 * Remeasure before moving a target between them.
 */
export const longCommandTests = Object.freeze([
	'cli_windows_machine_hub',
	'cli_windows_worktree_menu',
	'cli_windows_idle_close',
]);

export const testParts = Object.freeze([
	'workspace',
	'client',
	'client-commands',
	'client-long-commands',
]);
