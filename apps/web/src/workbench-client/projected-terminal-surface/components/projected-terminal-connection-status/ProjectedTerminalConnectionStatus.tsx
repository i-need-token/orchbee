import { desktopTrayMessages, projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './ProjectedTerminalConnectionStatus.module.css';

/**
 * Why a terminal shows no live attachment: its own attachment is `recovering` or `disconnected`, or
 * the Session's Server is `reconnecting` while it replaces a lost Worker, or `given-up` on it.
 */
export type ProjectedTerminalConnectionState =
	| { readonly kind: 'disconnected'; readonly onRetry: () => void }
	| { readonly kind: 'given-up'; readonly onQuitAll: (() => void) | null }
	| { readonly kind: 'reconnecting' }
	| { readonly kind: 'recovering' };

interface ProjectedTerminalConnectionStatusProps {
	readonly state: ProjectedTerminalConnectionState;
}

/**
 * The row across the top of a terminal that has no live attachment, with the action that answers it:
 * Retry for its own lost attachment, Quit All once the Server gave up. A Server state also says what
 * happens to the programs and to what the person types.
 */
export function ProjectedTerminalConnectionStatus({
	state,
}: ProjectedTerminalConnectionStatusProps): ReactNode {
	const intl = useOrchbeeIntl();
	let label = projectedTerminalMessages.connectionRecovering;
	let description: string | null = null;
	let action: ReactNode = null;
	switch (state.kind) {
		case 'disconnected':
			label = projectedTerminalMessages.disconnected;
			action = (
				<Button
					data-slot='projected-terminal-retry-connection'
					onClick={state.onRetry}
					variant='outline'
				>
					{intl.formatMessage(projectedTerminalMessages.retryConnection)}
				</Button>
			);
			break;
		case 'given-up':
			label = projectedTerminalMessages.runtimeGivenUp;
			description = intl.formatMessage(projectedTerminalMessages.runtimeGivenUpDescription);
			action =
				state.onQuitAll === null ? null : (
					<Button
						data-slot='projected-terminal-quit-all'
						onClick={state.onQuitAll}
						variant='outline'
					>
						{intl.formatMessage(desktopTrayMessages.quitAll)}
					</Button>
				);
			break;
		case 'reconnecting':
			description = intl.formatMessage(projectedTerminalMessages.runtimeReconnectingDescription);
			break;
		case 'recovering':
			break;
	}
	return (
		<div data-projected-terminal-status='true' role='alert'>
			{description === null ? (
				<span>{intl.formatMessage(label)}</span>
			) : (
				<span className={styles['text']}>
					<span>{intl.formatMessage(label)}</span>
					<span className={styles['description']}>{description}</span>
				</span>
			)}
			{action}
		</div>
	);
}
