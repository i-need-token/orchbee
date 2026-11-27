import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchNotificationQueue } from './model/WorkbenchNotificationQueue';
import styles from './WorkbenchNotificationCards.module.css';

interface WorkbenchNotificationCardsProps {
	readonly queue: WorkbenchNotificationQueue;
}

/** The cards the queue shows, newest last; errors and warnings announce assertively. */
export function WorkbenchNotificationCards({
	queue,
}: WorkbenchNotificationCardsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const cards = React.useSyncExternalStore(queue.subscribe, queue.getSnapshot);
	if (cards.length === 0) return null;
	return (
		<div className={styles['cards']} data-slot='workbench-notification-cards'>
			{cards.map((card) => (
				<div
					className={styles['card']}
					data-severity={card.severity}
					key={card.id}
					role={card.severity === 'info' ? 'status' : 'alert'}
					onKeyDown={(event) => {
						if (event.key !== 'Escape') return;
						event.preventDefault();
						queue.dismiss(card.id);
					}}
				>
					<p className={styles['message']}>{card.message}</p>
					<div className={styles['actions']}>
						{card.buttons.map((label, index) => (
							<button
								className={styles['button']}
								key={label}
								type='button'
								onClick={() => queue.choose(card.id, index)}
							>
								{label}
							</button>
						))}
						<button
							aria-label={intl.formatMessage(workbenchMessages.notificationClose)}
							className={styles['close']}
							type='button'
							onClick={() => queue.dismiss(card.id)}
						>
							<ProductIcon name='close' />
						</button>
					</div>
				</div>
			))}
		</div>
	);
}
