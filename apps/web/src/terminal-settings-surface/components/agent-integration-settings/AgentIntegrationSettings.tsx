import { Button, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import type { AgentIntegrationManagementControl } from '../../../agent-integration-management';
import { SettingsSection } from '../settings-section/SettingsSection';
import { TerminalSettingsRow } from '../terminal-settings-row/TerminalSettingsRow';
import { useAgentIntegrationSettings } from './hooks/useAgentIntegrationSettings';
import type {
	AgentIntegrationSettingsRowView,
	AgentIntegrationSettingsView,
} from './model/agent-integration-settings-view';
import styles from './AgentIntegrationSettings.module.css';

export interface AgentIntegrationSettingsProps {
	readonly control: AgentIntegrationManagementControl;
}

export function AgentIntegrationSettings({
	control,
}: AgentIntegrationSettingsProps): React.ReactNode {
	const view = useAgentIntegrationSettings(control);
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
					<React.Fragment key={row.integrationId}>
						<TerminalSettingsRow
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
							<div className={styles['controls']}>
								<Button
									aria-label={row.explainLabel}
									disabled={row.explainDisabled}
									size='sm'
									variant='ghost'
									onClick={row.onExplain}
								>
									{view.explainText}
								</Button>
								<Switch
									aria-busy={row.pending || undefined}
									aria-label={row.toggleLabel}
									checked={row.checked}
									disabled={row.disabled}
									onCheckedChange={row.onCheckedChange}
								/>
							</div>
						</TerminalSettingsRow>
						{row.diagnosis === null ? null : <AgentIntegrationDiagnosis row={row} view={view} />}
					</React.Fragment>
				))}
			</SettingsSection>
			<section className={styles['remote']}>
				<h2 className={styles['remoteTitle']}>{view.remoteTitle}</h2>
				<p className={styles['remoteDescription']}>{view.remoteDescription}</p>
			</section>
		</div>
	);
}

interface AgentIntegrationDiagnosisProps {
	readonly row: AgentIntegrationSettingsRowView;
	readonly view: AgentIntegrationSettingsView;
}

/** The read-only diagnosis of one Agent, shown under its row until closed. */
function AgentIntegrationDiagnosis({ row, view }: AgentIntegrationDiagnosisProps): React.ReactNode {
	const { diagnosis } = row;
	if (diagnosis === null) return null;
	return (
		<section
			aria-busy={diagnosis.pending || undefined}
			aria-label={row.diagnosisTitle}
			className={styles['diagnosis']}
			data-slot='agent-integration-diagnosis'
		>
			<div className={styles['diagnosisHeader']}>
				<h3 className={styles['diagnosisTitle']}>{row.diagnosisTitle}</h3>
				<Button size='sm' variant='ghost' onClick={view.onCloseDiagnosis}>
					{view.closeDiagnosisLabel}
				</Button>
			</div>
			<p aria-live='polite' className={styles['status']} data-tone={diagnosis.tone}>
				{diagnosis.headline}
			</p>
			{diagnosis.detail === null ? null : (
				<p className={styles['diagnosisText']}>{diagnosis.detail}</p>
			)}
			{diagnosis.facts.length === 0 ? null : (
				<dl className={styles['facts']}>
					{diagnosis.facts.map((fact) => (
						<React.Fragment key={fact.label}>
							<dt className={styles['factLabel']}>{fact.label}</dt>
							<dd className={styles['factValue']}>{fact.value}</dd>
						</React.Fragment>
					))}
				</dl>
			)}
			{diagnosis.pending ? null : <p className={styles['diagnosisText']}>{diagnosis.readOnly}</p>}
		</section>
	);
}
