import { DESKTOP_SESSION_REJOIN_MILLISECONDS } from '@orchbee/desktop-client-protocol';
import {
	projectedTerminalMessages,
	sessionConnectionMessages,
	useOrchbeeIntl,
} from '@orchbee/i18n';
import { Button, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import { useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

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
 * Stays once the window's Session connection ends, as the TUI keeps its last state beside its
 * connection status. While Main waits for a Server to serve the Session again, which reloads the
 * window when one does, it says Reconnecting; once that wait is over, Disconnected. Reconnect is the
 * person's own start at any time. It sits at the bottom center because Workbench notices and a
 * terminal's control status hold the bottom end.
 */
export function SessionDisconnectedStatus({
	ended,
	onReconnect,
}: SessionDisconnectedStatusProps): ReactNode {
	const intl = useOrchbeeIntl();
	const disconnected = useSyncExternalStore(ended.subscribe, ended.getSnapshot);
	const [waitOver, setWaitOver] = useState(false);
	useEffect(() => {
		if (!disconnected) return undefined;
		const timer = setTimeout(() => setWaitOver(true), DESKTOP_SESSION_REJOIN_MILLISECONDS);
		return () => clearTimeout(timer);
	}, [disconnected]);
	if (!disconnected) return null;
	return (
		<div className={styles['root']} data-slot='session-disconnected-status' role='status'>
			<span className={styles['label']}>
				{intl.formatMessage(
					waitOver
						? sessionConnectionMessages.disconnected
						: projectedTerminalMessages.connectionRecovering,
				)}
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
