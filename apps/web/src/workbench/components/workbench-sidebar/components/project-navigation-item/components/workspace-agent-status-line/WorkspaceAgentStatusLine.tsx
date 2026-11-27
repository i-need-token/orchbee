import { Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';
import type { ReactNode } from 'react';

import { WorkbenchAgentStateGlyph } from '../../../../../workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import type { WorkbenchWorkspaceAgentOverviewView } from '../../../../../../types/workbench-agent-overview';
import styles from './WorkspaceAgentStatusLine.module.css';

interface WorkspaceAgentStatusLineProps {
	readonly navigateAgent: (presentationId: TerminalPresentationId) => void;
	readonly overview: WorkbenchWorkspaceAgentOverviewView;
}

/**
 * A folded Workspace's status lines: every Agent's state as a 12 px glyph, most urgent first,
 * wrapping onto further lines so each one shows. A click on a glyph opens that Agent's terminal, as
 * its expanded row does, and its tooltip names that Agent alone: its state word, then its identity.
 */
export function WorkspaceAgentStatusLine({
	navigateAgent,
	overview,
}: WorkspaceAgentStatusLineProps): ReactNode {
	return (
		<span className={styles['line']} data-slot='workbench-workspace-agent-status'>
			{overview.foldedStatus.glyphs.map((glyph) => (
				<Tooltip key={glyph.presentationId}>
					<TooltipTrigger
						render={
							<button
								aria-label={glyph.accessibleLabel}
								className={styles['glyph']}
								data-navigation-reorder-excluded='true'
								tabIndex={-1}
								type='button'
								onClick={() => navigateAgent(glyph.presentationId)}
							/>
						}
					>
						<WorkbenchAgentStateGlyph status={glyph} />
					</TooltipTrigger>
					<TooltipContent align='start' side='bottom'>
						<span className={styles['agent']}>
							<WorkbenchAgentStateGlyph status={glyph} />
							<span className={styles['agentState']}>{glyph.status}</span>
							<span className={styles['agentIdentity']}>{glyph.identity}</span>
						</span>
					</TooltipContent>
				</Tooltip>
			))}
		</span>
	);
}
