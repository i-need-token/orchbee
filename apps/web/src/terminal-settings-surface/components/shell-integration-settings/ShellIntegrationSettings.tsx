import { Button, Input, NativeSelect, NativeSelectOption } from '@orchbee/ui/components';
import type * as React from 'react';

import type { TerminalFeedbackSettingsController } from '../../../terminal-feedback-settings';
import { ShellSetupCard } from './components/shell-setup-card/ShellSetupCard';
import { useShellIntegrationSettings } from './hooks/useShellIntegrationSettings';
import styles from './ShellIntegrationSettings.module.css';

export interface ShellIntegrationSettingsProps {
	readonly controller: TerminalFeedbackSettingsController;
	readonly query: string;
}

export function ShellIntegrationSettings({
	controller,
	query,
}: ShellIntegrationSettingsProps): React.ReactNode {
	const view = useShellIntegrationSettings(controller, query);

	return (
		<div className={styles['root']} data-slot='shell-integration-settings'>
			{view.persistenceWarning === null ? null : (
				<p className={styles['warning']} role='alert'>
					{view.persistenceWarning}
				</p>
			)}
			{view.manualNotice === null ? null : <p className={styles['notice']}>{view.manualNotice}</p>}
			{view.setupCards.length === 0 ? null : (
				<div className={styles['setupGroup']}>
					{view.setupCards.map((card) => (
						<ShellSetupCard
							key={card.id}
							command={card.command}
							commandLabel={card.commandLabel}
							description={card.description}
							title={card.title}
						/>
					))}
				</div>
			)}
			{view.reminder === null ? null : (
				<section className={styles['card']}>
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
			)}
			{view.notification === null ? null : (
				<section className={styles['card']}>
					<h2 className={styles['title']}>{view.notification.title}</h2>
					<p className={styles['description']}>{view.notification.description}</p>
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
			)}
			{view.reset === null ? null : (
				<section className={styles['card']}>
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
			)}
			{view.noResults === null ? null : <p className={styles['empty']}>{view.noResults}</p>}
		</div>
	);
}
