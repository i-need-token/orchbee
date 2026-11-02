import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalViewOnly.module.css';

interface ProjectedTerminalViewOnlyProps {
	/** The pane is smaller than the terminal and follows its cursor, as the TUI's live crop does. */
	readonly cropped: boolean;
	readonly onTakeControl: () => void;
	readonly taking: boolean;
}

/**
 * Stays while another Client holds the terminal, as the TUI keeps read-only input explicit. It sits
 * at the bottom so the alert a refused request raises at the top never covers the way to ask again;
 * the surface keeps the terminal's rows clear of it while it says View only.
 */
export function ProjectedTerminalViewOnly({
	cropped,
	onTakeControl,
	taking,
}: ProjectedTerminalViewOnlyProps): ReactNode {
	const intl = useOrchbeeIntl();
	let label = projectedTerminalMessages.controlViewOnly;
	if (taking) label = projectedTerminalMessages.controlTaking;
	else if (cropped) label = projectedTerminalMessages.controlViewOnlyCropped;
	return (
		<div className={styles['root']} data-slot='projected-terminal-view-only'>
			<span className={styles['label']}>{intl.formatMessage(label)}</span>
			<Tooltip>
				<TooltipTrigger
					render={
						<Button
							data-slot='projected-terminal-take-control'
							disabled={taking}
							onClick={onTakeControl}
							size='xs'
							variant='outline'
						/>
					}
				>
					{intl.formatMessage(projectedTerminalMessages.controlTake)}
				</TooltipTrigger>
				<TooltipContent side='top'>
					{intl.formatMessage(
						cropped
							? projectedTerminalMessages.controlViewOnlyCroppedReason
							: projectedTerminalMessages.controlViewOnlyReason,
					)}
				</TooltipContent>
			</Tooltip>
		</div>
	);
}
