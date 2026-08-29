import { Button, ProductIcon } from '@orchbee/ui/components';
import type * as React from 'react';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../agent-integration-management';
import { useAgentIntegrationReminder } from './hooks/useAgentIntegrationReminder';
import styles from './AgentIntegrationReminder.module.css';

export interface AgentIntegrationReminderProps {
	readonly control: AgentIntegrationManagementControl;
	readonly controller: AgentIntegrationReminderController;
	readonly onReview: (returnFocus: HTMLElement) => void;
}

export function AgentIntegrationReminder({
	control,
	controller,
	onReview,
}: AgentIntegrationReminderProps): React.ReactNode {
	const view = useAgentIntegrationReminder(control, controller, onReview);
	if (view === null) return null;
	return (
		<aside
			aria-labelledby='agent-integration-reminder-title'
			aria-live='polite'
			className={styles['root']}
			data-agent-integration={view.integrationId}
			data-slot='agent-integration-reminder'
			role='status'
		>
			<div className={styles['leadingIcon']}>
				<ProductIcon name='agent' />
			</div>
			<div className={styles['content']}>
				<strong className={styles['title']} id='agent-integration-reminder-title'>
					{view.title}
				</strong>
				<p className={styles['body']}>{view.body}</p>
				<Button
					size='xs'
					variant='ghost'
					onClick={(event): void => view.onReview(event.currentTarget)}
				>
					{view.reviewLabel}
				</Button>
			</div>
			<Button
				aria-label={view.dismissLabel}
				className={styles['dismiss']}
				size='icon-sm'
				title={view.dismissLabel}
				variant='ghost'
				onClick={view.onDismiss}
			>
				<ProductIcon name='close' />
			</Button>
		</aside>
	);
}
