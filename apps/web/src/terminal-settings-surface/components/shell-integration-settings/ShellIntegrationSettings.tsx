import { Button, Input, NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import type { TerminalFeedbackSettingsController } from '../../../terminal-feedback-settings';
import { ShellSetupCard } from './components/shell-setup-card/ShellSetupCard';
import { useShellIntegrationSettings } from './hooks/useShellIntegrationSettings';
import styles from './ShellIntegrationSettings.module.css';

export interface ShellIntegrationSettingsProps {
	readonly controller: TerminalFeedbackSettingsController;
}

export function ShellIntegrationSettings({
	controller,
}: ShellIntegrationSettingsProps): React.ReactNode {
	const view = useShellIntegrationSettings(controller);

	return (
		<div className={styles['root']} data-slot='shell-integration-settings'>
			{view.persistenceWarning === null ? null : (
				<p className={styles['warning']} role='alert'>
					{view.persistenceWarning}
				</p>
			)}
			<p className={styles['notice']}>{view.manualNotice}</p>
			<div className={styles['setupGroup']}>
				{view.setupCards.map((card) => (
					<ShellSetupCard
						key={card.id}
						command={card.command}
						commandLabel={card.commandLabel}
						description={card.description}
						settingId={`shell-integration.${card.id}`}
						title={card.title}
					/>
				))}
			</div>
			<section className={styles['card']} id='shell-integration.completion-reminders'>
				<h2 className={styles['title']}>{view.reminder.title}</h2>
				<p className={styles['description']}>{view.reminder.description}</p>
				<label className={styles['field']}>
					<span className={styles['label']}>{view.reminder.modeLabel}</span>
					<NativeSelect
						data-slot='completion-reminder-mode'
						value={view.reminder.modeValue}
						onChange={view.reminder.onModeChange}
					>
						{view.reminder.modeOptions.map((option) => (
							<NativeSelectOption key={option.value} value={option.value}>
								{option.label}
							</NativeSelectOption>
						))}
					</NativeSelect>
				</label>
				<label className={styles['field']}>
					<span className={styles['label']}>{view.reminder.minimumLabel}</span>
					<span className={styles['description']}>{view.reminder.minimumDescription}</span>
					<Input
						aria-invalid={view.reminder.minimumInvalid}
						data-slot='completion-reminder-minimum'
						max={view.reminder.minimumMaximum}
						min={view.reminder.minimumMinimum}
						type='number'
						value={view.reminder.minimumDraft}
						onBlur={view.reminder.onMinimumBlur}
						onChange={view.reminder.onMinimumChange}
						onKeyDown={view.reminder.onMinimumKeyDown}
					/>
					{view.reminder.minimumInvalid ? (
						<span className={styles['error']} role='alert'>
							{view.reminder.minimumInvalidMessage}
						</span>
					) : null}
				</label>
			</section>
			<section className={styles['card']} id='shell-integration.notifications'>
				<h2 className={styles['title']}>{view.notification.title}</h2>
				<p className={styles['description']}>{view.notification.description}</p>
				<label className={styles['field']}>
					<span className={styles['label']}>{view.notification.agentModeLabel}</span>
					<NativeSelect
						data-slot='agent-notification-mode'
						value={view.notification.agentModeValue}
						onChange={view.notification.onAgentModeChange}
					>
						{view.notification.agentModeOptions.map((option) => (
							<NativeSelectOption key={option.value} value={option.value}>
								{option.label}
							</NativeSelectOption>
						))}
					</NativeSelect>
				</label>
				<p className={styles['status']} data-permission={view.notification.permission}>
					{view.notification.status}
				</p>
				{view.notification.onRequestPermission === null ? null : (
					<Button
						data-slot='request-terminal-notification-permission'
						variant='outline'
						onClick={view.notification.onRequestPermission}
					>
						{view.notification.requestLabel}
					</Button>
				)}
			</section>
			<section className={styles['card']} id='shell-integration.reset'>
				<h2 className={styles['title']}>{view.reset.label}</h2>
				<p className={styles['description']}>{view.reset.description}</p>
				<Button
					data-slot='reset-terminal-feedback-settings'
					variant='outline'
					onClick={view.reset.onReset}
				>
					{view.reset.label}
				</Button>
			</section>
		</div>
	);
}
