import { sessionConnectionMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import { useSyncExternalStore, type ReactNode } from 'react';

import styles from './SessionDisconnectedStatus.module.css';

/** Whether this window's Session connection has ended. Both functions keep their identity. */
interface SessionConnectionEndSource {
	readonly getSnapshot: () => boolean;
	readonly subscribe: (listener: () => void) => () => void;
}

export interface SessionDisconnectedStatusProps {
	readonly ended: SessionConnectionEndSource;
	readonly onReconnect: () => void;
}

/**
 * Stays once the window's Session connection ends, as the TUI keeps its last state beside a
 * persistent Disconnected status. Nothing reconnects by itself: the person decides when. It sits at
 * the bottom center because Workbench notices and a terminal's control status hold the bottom end.
 */
export function SessionDisconnectedStatus({
	ended,
	onReconnect,
}: SessionDisconnectedStatusProps): ReactNode {
	const intl = useOrchbeeIntl();
	const disconnected = useSyncExternalStore(ended.subscribe, ended.getSnapshot);
	if (!disconnected) return null;
	return (
		<div className={styles['root']} data-slot='session-disconnected-status' role='status'>
			<span className={styles['label']}>
				{intl.formatMessage(sessionConnectionMessages.disconnected)}
			</span>
			<Tooltip>
				<TooltipTrigger
					render={
						<Button
							data-slot='session-reconnect'
							onClick={onReconnect}
							size='xs'
							variant='outline'
						/>
					}
				>
					{intl.formatMessage(sessionConnectionMessages.reconnect)}
				</TooltipTrigger>
				<TooltipContent side='top'>
					{intl.formatMessage(sessionConnectionMessages.reconnectDescription)}
				</TooltipContent>
			</Tooltip>
		</div>
	);
}
