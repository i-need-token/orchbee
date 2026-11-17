import { sessionConnectionMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './WorkbenchFailure.module.css';

export interface WorkbenchFailureProps {
	readonly onReload: () => void;
}

/**
 * Takes the place of a Workbench that failed to render outside any one terminal. The Session and
 * its terminals keep running, so reloading the window, as Reconnect does, shows them again; nothing
 * reloads by itself. The window keeps a title-bar strip to be moved by, since no Workbench draws one.
 */
export function WorkbenchFailure({ onReload }: WorkbenchFailureProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<main className={styles['root']} data-slot='workbench-failure'>
			<div aria-hidden='true' className={styles['dragRegion']} />
			<div className={styles['content']}>
				<h1 className={styles['title']}>
					{intl.formatMessage(sessionConnectionMessages.workbenchFailedTitle)}
				</h1>
				<p className={styles['description']}>
					{intl.formatMessage(sessionConnectionMessages.workbenchFailedDescription)}
				</p>
				<Button
					className={styles['action']}
					data-slot='workbench-failure-reload'
					onClick={onReload}
					variant='outline'
				>
					{intl.formatMessage(sessionConnectionMessages.workbenchFailedReload)}
				</Button>
			</div>
		</main>
	);
}
