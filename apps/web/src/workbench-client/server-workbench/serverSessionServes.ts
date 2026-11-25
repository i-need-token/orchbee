/**
 * The one table from a `client-server` capability to the window actions that need it, as the TUI's
 * `session_capabilities.rs` keeps its own. A Session whose handshake lacks a capability disables
 * only those actions, which show as unavailable with their reason and never send their request;
 * every other action and the connection stay.
 */

/** A window action that only a Session listing a certain capability can serve. */
type ServerSessionAction =
	| 'check-for-updates'
	| 'follow-agent-confirmations'
	| 'follow-terminal-runtime'
	| 'read-workspace-files';

/** Every capability this build's `client-server` offer lists, with the window actions that need it. */
const NEEDED_BY: readonly (readonly [string, readonly ServerSessionAction[]])[] = [
	// The window reads and changes how the Server answers Agent confirmations only where it is listed.
	['agent-confirmation', ['follow-agent-confirmations']],
	// Only the CLI's recent Agent read asks for the input area.
	['agent-input-area', []],
	// Main's Update reads it itself, before a switch.
	['compatibility-window', []],
	// Only the TUI's start in the focused Panel needs it, which the window lacks.
	['prompt-gated-input', []],
	// The terminal surface follows the Session's terminal-runtime read only where it is listed.
	['terminal-runtime', ['follow-terminal-runtime']],
	['update-check', ['check-for-updates']],
	// The right bar is offered only where the Server reads Workspace files.
	['workspace-files', ['read-workspace-files']],
];

/**
 * Whether `action` is available under the capabilities the Session's handshake negotiated. Unlike
 * the TUI, which reads its own handshake, the window knows them only from Main's start answer, so
 * an unknown answer (null) serves nothing: a request a Session may not know is never sent.
 */
export function serverSessionServes(
	negotiated: readonly string[] | null,
	action: ServerSessionAction,
): boolean {
	if (negotiated === null) return false;
	return NEEDED_BY.every(
		([capability, actions]) => !actions.includes(action) || negotiated.includes(capability),
	);
}
