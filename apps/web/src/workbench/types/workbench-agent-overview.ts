import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchAgentStatusView } from '../utils/formatWorkbenchAgentStatus';
import type { WorkbenchAgentConfirmationChoice } from './workbench-agent-confirmations';

/** The Agent in the active Tab whose confirmations the Server offers to answer. */
export interface WorkbenchAgentConfirmationOfferView {
	/** The Agent's product name. */
	readonly agentLabel: string;
	readonly presentationId: TerminalPresentationId;
}

interface WorkbenchWorkspaceAgentRowView {
	readonly accessibleLabel: string;
	/** The Agent's product name, or a custom report's own name. */
	readonly agentLabel: string;
	/**
	 * How Orchbee answers this Agent's routine confirmations, with the answer that names its type;
	 * absent where the Server answers none for it.
	 */
	readonly confirmation?:
		| { readonly agentTypeLabel: string; readonly choice: WorkbenchAgentConfirmationChoice }
		| undefined;
	/** Whether this Agent's terminal is the focused Panel's in the open Workspace. */
	readonly current: boolean;
	/** The Panel holding the Agent's terminal; Rename and Close name it. */
	readonly panelId: string;
	readonly presentationId: TerminalPresentationId;
	/** Whether the terminal is its Panel's primary one, the only one Panel rename names. */
	readonly primary: boolean;
	readonly status: WorkbenchAgentStatusView;
	/** The state with why the confirmation the Agent shows waits for the person, where one does. */
	readonly statusReason?: string | undefined;
	readonly tabId: string;
	readonly terminalContextId: string;
	/** What names the Agent before its provider, by the TUI's rule; empty leaves the provider alone. */
	readonly titleLabel: string;
	readonly unreadLabel?: string | undefined;
}

/** A folded Workspace's second line: its Agents' states, most urgent first. */
interface WorkbenchWorkspaceAgentFoldedStatusView {
	/**
	 * Every Agent's state, keyed by its terminal Presentation. The line shows as many leading glyphs
	 * as it has room for and counts the rest as `+N`.
	 */
	readonly glyphs: readonly (Pick<WorkbenchAgentStatusView, 'shape' | 'status' | 'tone'> & {
		/** The Agent's identity as its expanded row shows it: its title and provider. */
		readonly identity: string;
		readonly presentationId: TerminalPresentationId;
	})[];
	/** Each distinct state in words, most urgent first. */
	readonly label: string;
	/** Each distinct state once, most urgent first, as the line's tooltip lists them. */
	readonly legend: readonly WorkbenchAgentStatusView[];
}

export interface WorkbenchWorkspaceAgentOverviewView {
	readonly agentLabel: string;
	readonly collapseLabel: string;
	readonly countLabel: string;
	readonly expandLabel: string;
	/** Whether the Agent rows show beneath the Workspace; otherwise its second line lists their states. */
	readonly expanded: boolean;
	readonly foldedStatus: WorkbenchWorkspaceAgentFoldedStatusView;
	readonly projectId: string | null;
	readonly rows: readonly WorkbenchWorkspaceAgentRowView[];
	/** The most urgent row state. */
	readonly status: WorkbenchAgentStatusView;
	readonly unreadLabel?: string | undefined;
	readonly workspaceId: string;
}
