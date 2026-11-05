import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import styles from './WorkspaceAgentStatusLine.module.css';

interface WorkspaceAgentStatusLineProps {
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
}

/**
 * A folded Workspace's second line: its Agents' states as 12 px glyphs, most urgent first, then
 * `+N` for the rest. Its tooltip names each state once, and Unread while the row repeats it.
 */
export function WorkspaceAgentStatusLine({ overview }: WorkspaceAgentStatusLineProps): ReactNode {
	const status = overview.foldedStatus;
	return (
		<Tooltip>
			<TooltipTrigger
				render={<span className={styles['line']} data-slot='workbench-workspace-agent-status' />}
			>
				{status.glyphs.map((glyph) => (
					<WorkbenchAgentStateGlyph key={glyph.presentationId} size='small' status={glyph} />
				))}
				{status.remainder > 0 ? <span className={styles['more']}>+{status.remainder}</span> : null}
			</TooltipTrigger>
			<TooltipContent align='start' side='bottom'>
				<span className={styles['legend']}>
					{status.legend.map((state) => (
						<span key={state.status} className={styles['legendRow']}>
							<WorkbenchAgentStateGlyph size='small' status={state} />
							{state.status}
						</span>
					))}
					{overview.unreadLabel === undefined ? null : (
						<span className={styles['legendRow']}>
							<span aria-hidden='true' className={styles['unread']} />
							{overview.unreadLabel}
						</span>
					)}
				</span>
			</TooltipContent>
		</Tooltip>
	);
}
