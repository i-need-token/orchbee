import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import type { AgentIntegrationManagementControl } from '../../../agent-integration-management';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SETTINGS_LIST_FILTER_THRESHOLD } from '../../constants/settings-list-filter';
import { SettingsListFilter } from '../settings-list-filter/SettingsListFilter';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import { SettingsRow } from '../settings-row/SettingsRow';
import { useAgentIntegrationSettings } from './hooks/useAgentIntegrationSettings';
import type {
	AgentIntegrationSettingsRowView,
	AgentIntegrationSettingsView,
} from './model/agent-integration-settings-view';
import styles from './AgentIntegrationSettings.module.css';

export interface AgentIntegrationSettingsProps {
	readonly control: AgentIntegrationManagementControl;
	readonly query: string;
}

/** Agent integrations as one list card: each Agent's state as its description and a switch. */
export function AgentIntegrationSettings({
	control,
	query,
}: AgentIntegrationSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useAgentIntegrationSettings(control);
	const [filter, setFilter] = React.useState('');
	const searching = query.trim() !== '';
	const filterVisible = !searching && view.rows.length > SETTINGS_LIST_FILTER_THRESHOLD;
	const rows = filterVisible
		? view.rows.filter((row) =>
				matchesSettingsQuery(filter, [row.agentName, row.status], intl.locale),
			)
		: view.rows;
	return (
		<SettingsGroup
			action={
				<Button
					disabled={view.refreshDisabled}
					size='xs'
					type='button'
					variant='ghost'
					onClick={view.onRefresh}
				>
					{view.refreshLabel}
				</Button>
			}
			description={intl.formatMessage(
				terminalSettingsMessages.pageGroupAgentIntegrationsDescription,
			)}
			label={intl.formatMessage(terminalSettingsMessages.pageGroupAgentIntegrations)}
		>
			{searching || view.failure === null ? null : (
				<p className={styles['failure']} data-settings-row role='alert'>
					{view.failure}
				</p>
			)}
			{filterVisible ? (
				<SettingsListFilter
					placeholder={intl.formatMessage(terminalSettingsMessages.pageFilterAgents)}
					shown={rows.length}
					total={view.rows.length}
					value={filter}
					onChange={setFilter}
				/>
			) : null}
			{filterVisible && rows.length === 0 ? <SettingsListFilterEmpty /> : null}
			{rows.map((row) => (
				<React.Fragment key={row.integrationId}>
					<SettingsRow
						description={row.status}
						descriptionTone={row.statusTone}
						label={row.agentName}
						query={query}
						searchTerms={[row.description]}
						settingId={row.settingId}
					>
						<Button
							aria-label={row.explainLabel}
							className={styles['explain']}
							disabled={row.explainDisabled}
							size='xs'
							type='button'
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
					</SettingsRow>
					{row.diagnosis === null ? null : <AgentIntegrationDiagnosis row={row} view={view} />}
				</React.Fragment>
			))}
		</SettingsGroup>
	);
}

interface AgentIntegrationDiagnosisProps {
	readonly row: AgentIntegrationSettingsRowView;
	readonly view: AgentIntegrationSettingsView;
}

/** The read-only diagnosis of one Agent, shown under its line until closed. */
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
				<Button size='xs' type='button' variant='ghost' onClick={view.onCloseDiagnosis}>
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
