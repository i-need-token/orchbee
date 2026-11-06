import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchAgentStatusView } from '../utils/formatWorkbenchAgentStatus';

interface WorkbenchWorkspaceAgentRowView {
	readonly accessibleLabel: string;
	/** The Agent's product name, or a custom report's own name. */
	readonly agentLabel: string;
	/** Whether this Agent's terminal is the focused Panel's in the open Workspace. */
	readonly current: boolean;
	readonly presentationId: TerminalPresentationId;
	readonly status: WorkbenchAgentStatusView;
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
	readonly glyphs: readonly (Pick<WorkbenchAgentStatusView, 'shape' | 'tone'> & {
		/** The Agent's name as its expanded row shows it; the `+N` list names the Agents it hides. */
		readonly label: string;
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
