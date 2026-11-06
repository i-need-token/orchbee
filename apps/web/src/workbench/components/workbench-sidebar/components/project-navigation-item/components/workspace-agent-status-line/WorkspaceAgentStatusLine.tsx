import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import { useWorkspaceAgentStatusFit } from './useWorkspaceAgentStatusFit';
import styles from './WorkspaceAgentStatusLine.module.css';

interface WorkspaceAgentStatusLineProps {
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
}

/**
 * A folded Workspace's second line: its Agents' states as 12 px glyphs, most urgent first, as many
 * as the line has room for, then `+N` for the rest. The glyphs' tooltip names each state once, and
 * the `+N` tooltip lists the Agents it hides in the same order.
 */
export function WorkspaceAgentStatusLine({ overview }: WorkspaceAgentStatusLineProps): ReactNode {
	const status = overview.foldedStatus;
	const { fit, glyphMeasureRef, lineRef, remainderMeasureRef } = useWorkspaceAgentStatusFit(
		status.glyphs.length,
	);
	const shownGlyphs = status.glyphs.slice(0, fit.shown);
	const hiddenGlyphs = status.glyphs.slice(fit.shown, fit.shown + fit.hidden);
	return (
		<span className={styles['line']} data-slot='workbench-workspace-agent-status' ref={lineRef}>
			{shownGlyphs.length === 0 ? null : (
				<Tooltip>
					<TooltipTrigger render={<span className={styles['glyphs']} />}>
						{shownGlyphs.map((glyph) => (
							<WorkbenchAgentStateGlyph key={glyph.presentationId} size='small' status={glyph} />
						))}
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
			)}
			{hiddenGlyphs.length === 0 ? null : (
				<Tooltip>
					<TooltipTrigger
						render={
							<span className={styles['more']} data-slot='workbench-workspace-agent-status-more' />
						}
					>
						+{fit.hidden}
					</TooltipTrigger>
					<TooltipContent align='start' side='bottom'>
						<span className={styles['roster']}>
							{hiddenGlyphs.map((glyph) => (
								<span key={glyph.presentationId} className={styles['rosterRow']}>
									<WorkbenchAgentStateGlyph size='small' status={glyph} />
									<span className={styles['rosterName']}>{glyph.label}</span>
								</span>
							))}
						</span>
					</TooltipContent>
				</Tooltip>
			)}
			<span aria-hidden='true' className={styles['measure']}>
				<span className={styles['measureGlyph']} ref={glyphMeasureRef} />
				<span className={styles['more']} ref={remainderMeasureRef} />
			</span>
		</span>
	);
}
