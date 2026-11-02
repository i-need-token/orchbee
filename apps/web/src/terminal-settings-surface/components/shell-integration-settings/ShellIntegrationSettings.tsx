import { Button, Input, NativeSelect, NativeSelectOption, Switch } from '@orchbee/ui/components';
import type * as React from 'react';

import type { TerminalFeedbackSettingsController } from '../../../terminal-feedback-settings';
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
					<span className={styles['description']} id='completion-reminder-minimum-description'>
						{view.reminder.minimumDescription}
					</span>
					<Input
						aria-describedby={
							view.reminder.minimumInvalid
								? 'completion-reminder-minimum-description completion-reminder-minimum-error'
								: 'completion-reminder-minimum-description'
						}
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
						<span className={styles['error']} id='completion-reminder-minimum-error' role='alert'>
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
				<label className={styles['field']} id='shell-integration.notification-sound'>
					<span className={styles['label']}>{view.notification.soundLabel}</span>
					<span className={styles['description']}>{view.notification.soundDescription}</span>
					<Switch
						aria-label={view.notification.soundLabel}
						checked={view.notification.soundEnabled}
						onCheckedChange={view.notification.onSoundChange}
					/>
				</label>
				{view.notification.onRequestPermission === null ? null : (
					<Button
						data-slot='request-terminal-notification-permission'
						type='button'
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
					type='button'
					variant='outline'
					onClick={view.reset.onReset}
				>
					{view.reset.label}
				</Button>
			</section>
		</div>
	);
}
