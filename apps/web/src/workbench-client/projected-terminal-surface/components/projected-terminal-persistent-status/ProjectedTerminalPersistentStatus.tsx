import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { ProjectedTerminalOutcome } from '../../../projected-terminal-renderer';
import styles from './ProjectedTerminalPersistentStatus.module.css';

type ProjectedTerminalPersistentState =
	| {
			readonly kind: 'ended';
			readonly onCreateTab: () => void;
			readonly outcome: ProjectedTerminalOutcome;
	  }
	| {
			/** The pane is smaller than the terminal and follows its cursor, as the TUI's live crop does. */
			readonly cropped: boolean;
			readonly kind: 'view-only';
			readonly onTakeControl: () => void;
			readonly taking: boolean;
	  };

interface ProjectedTerminalPersistentStatusProps {
	readonly state: ProjectedTerminalPersistentState;
}

/**
 * Names a lasting state in a word or two beside the action that answers it, as the TUI's floating
 * status does: View only beside Take control while another Client holds the terminal, and Terminal
 * exited or Terminal failed beside New Tab once it has ended. It sits at the bottom so the alert a
 * refused request raises at the top never covers the way to ask again. The surface keeps the
 * terminal's rows clear of it while it says View only; an ended terminal's last row may sit behind
 * it, as behind the TUI's status.
 */
export function ProjectedTerminalPersistentStatus({
	state,
}: ProjectedTerminalPersistentStatusProps): ReactNode {
	const intl = useOrchbeeIntl();
	if (state.kind === 'ended') {
		const failed = state.outcome === 'failed';
		return (
			<div className={styles['root']} data-slot='projected-terminal-ended'>
				<span className={styles['label']} role={failed ? 'alert' : 'status'}>
					{intl.formatMessage(
						failed
							? projectedTerminalMessages.runtimeFailure
							: projectedTerminalMessages.terminalExited,
					)}
				</span>
				<Button
					data-slot='projected-terminal-new-tab'
					onClick={state.onCreateTab}
					size='xs'
					variant='outline'
				>
					{intl.formatMessage(projectedTerminalMessages.newTab)}
				</Button>
			</div>
		);
	}
	let label = projectedTerminalMessages.controlViewOnly;
	if (state.taking) label = projectedTerminalMessages.controlTaking;
	else if (state.cropped) label = projectedTerminalMessages.controlViewOnlyCropped;
	return (
		<div className={styles['root']} data-slot='projected-terminal-view-only'>
			<span className={styles['label']}>{intl.formatMessage(label)}</span>
			<Tooltip>
				<TooltipTrigger
					render={
						<Button
							data-slot='projected-terminal-take-control'
							disabled={state.taking}
							onClick={state.onTakeControl}
							size='xs'
							variant='outline'
						/>
					}
				>
					{intl.formatMessage(projectedTerminalMessages.controlTake)}
				</TooltipTrigger>
				<TooltipContent side='top'>
					{intl.formatMessage(
						state.cropped
							? projectedTerminalMessages.controlViewOnlyCroppedReason
							: projectedTerminalMessages.controlViewOnlyReason,
					)}
				</TooltipContent>
			</Tooltip>
		</div>
	);
}
