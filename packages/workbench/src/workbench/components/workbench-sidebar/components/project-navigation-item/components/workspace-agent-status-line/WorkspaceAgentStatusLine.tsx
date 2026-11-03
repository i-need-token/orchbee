import type { ReactNode } from 'react';

import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import styles from './WorkspaceAgentStatusLine.module.css';

interface WorkspaceAgentStatusLineProps {
	readonly status: WorkbenchWorkspaceAgentOverviewView['foldedStatus'];
}

/** A folded Workspace's Agent states, most urgent first, with any states past them counted. */
export function WorkspaceAgentStatusLine({ status }: WorkspaceAgentStatusLineProps): ReactNode {
	return (
		<span
			aria-hidden='true'
			className={styles['line']}
			data-slot='workbench-sidebar-workspace-agent-states'
			title={status.label}
		>
			{status.glyphs.map((glyph) => (
				<WorkbenchAgentStateGlyph key={glyph.presentationId} status={glyph} />
			))}
			{status.remainder > 0 ? (
				<span className={styles['remainder']}>+{status.remainder}</span>
			) : null}
		</span>
	);
}
