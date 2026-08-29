import { Button } from '@orchbee/ui/components';
import type * as React from 'react';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../../../agent-integration-management';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import { useAgentIntegrationSettings } from './hooks/useAgentIntegrationSettings';
import styles from './AgentIntegrationSettings.module.css';

export interface AgentIntegrationSettingsProps {
	readonly control: AgentIntegrationManagementControl;
	readonly reminderController?: AgentIntegrationReminderController | undefined;
}

export function AgentIntegrationSettings({
	control,
	reminderController,
}: AgentIntegrationSettingsProps): React.ReactNode {
	const view = useAgentIntegrationSettings(control, reminderController);
	return (
		<div className={styles['root']} data-slot='agent-integration-settings'>
			<div className={styles['toolbar']}>
				<p className={styles['disclosure']}>{view.disclosure}</p>
				<Button disabled={view.refreshDisabled} variant='outline' onClick={view.onRefresh}>
					{view.refreshLabel}
				</Button>
			</div>
			{view.failure === null ? null : (
				<p className={styles['failure']} role='alert'>
					{view.failure}
				</p>
			)}
			<SettingsSection title={view.localSection} visible>
				{view.rows.map((row) => (
					<TerminalSettingsRow
						key={row.integrationId}
						description={row.description}
						label={row.agentName}
						settingId={row.settingId}
						status={
							<span className={styles['status']} data-tone={row.statusTone}>
								{row.status}
							</span>
						}
						visible
					>
						<Button disabled={row.actionDisabled} variant='outline' onClick={row.onAction}>
							{row.actionLabel}
						</Button>
					</TerminalSettingsRow>
				))}
			</SettingsSection>
			<section className={styles['remote']}>
				<h2 className={styles['remoteTitle']}>{view.remoteTitle}</h2>
				<p className={styles['remoteDescription']}>{view.remoteDescription}</p>
			</section>
		</div>
	);
}
