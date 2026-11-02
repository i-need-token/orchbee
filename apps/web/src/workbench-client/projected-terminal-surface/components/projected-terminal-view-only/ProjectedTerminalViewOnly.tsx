import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalViewOnly.module.css';

interface ProjectedTerminalViewOnlyProps {
	readonly onTakeControl: () => void;
	readonly taking: boolean;
}

/**
 * Stays while another Client holds the terminal, as the TUI keeps read-only input explicit. It sits
 * at the bottom so the alert a refused request raises at the top never covers the way to ask again.
 */
export function ProjectedTerminalViewOnly({
	onTakeControl,
	taking,
}: ProjectedTerminalViewOnlyProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-slot='projected-terminal-view-only'>
			<span className={styles['label']}>
				{intl.formatMessage(
					taking
						? projectedTerminalMessages.controlTaking
						: projectedTerminalMessages.controlViewOnly,
				)}
			</span>
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
					{intl.formatMessage(projectedTerminalMessages.controlViewOnlyReason)}
				</TooltipContent>
			</Tooltip>
		</div>
	);
}
