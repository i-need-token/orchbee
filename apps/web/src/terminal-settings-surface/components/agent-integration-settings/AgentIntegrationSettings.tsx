import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import type { AgentIntegrationManagementControl } from '../../../agent-integration-management';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsChildPage } from '../settings-child-page/SettingsChildPage';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SETTINGS_LIST_FILTER_THRESHOLD } from '../../constants/settings-list-filter';
import { SettingsListFilter } from '../settings-list-filter/SettingsListFilter';
import { SettingsListFilterEmpty } from '../settings-list-filter-empty/SettingsListFilterEmpty';
import { SettingsRow } from '../settings-row/SettingsRow';
import { useAgentIntegrationSettings } from './hooks/useAgentIntegrationSettings';
import type {
	AgentIntegrationDiagnosisView,
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
	// A search lists matches only; a diagnosis opens from the category's own list.
	const diagnosed = searching ? undefined : view.rows.find((row) => row.diagnosis !== null);
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
			{searching ? null : <SettingFailure message={view.failure} />}
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
				<SettingsRow
					key={row.integrationId}
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
			))}
			{diagnosed === undefined ? null : <AgentIntegrationDiagnosis row={diagnosed} view={view} />}
		</SettingsGroup>
	);
}

interface AgentIntegrationDiagnosisProps {
	readonly row: AgentIntegrationSettingsRowView;
	readonly view: AgentIntegrationSettingsView;
}

/**
 * One Agent's read-only diagnosis, or the question before a check that starts its program, on a
 * child page in place of the list; Explain there runs or repeats the check.
 */
function AgentIntegrationDiagnosis({ row, view }: AgentIntegrationDiagnosisProps): React.ReactNode {
	const { diagnosis } = row;
	if (diagnosis === null) return null;
	return (
		<SettingsChildPage title={row.diagnosisTitle} onBack={view.onCloseDiagnosis}>
			<div
				aria-busy={diagnosis.pending || undefined}
				className={styles['diagnosis']}
				data-slot='agent-integration-diagnosis'
			>
				<p aria-live='polite' className={styles['status']} data-tone={diagnosis.tone}>
					{diagnosis.headline}
				</p>
				{diagnosis.disclosed ? (
					<details>
						<summary className={styles['detailsSummary']}>{view.detailsLabel}</summary>
						<AgentIntegrationDiagnosisBody diagnosis={diagnosis} />
					</details>
				) : (
					<AgentIntegrationDiagnosisBody diagnosis={diagnosis} />
				)}
				<div className={styles['diagnosisActions']}>
					<Button
						aria-label={row.explainLabel}
						disabled={row.explainDisabled}
						size='sm'
						type='button'
						variant='outline'
						onClick={row.onExplain}
					>
						{view.explainText}
					</Button>
				</div>
			</div>
		</SettingsChildPage>
	);
}

/** What the check read beyond its one sentence, and what it changed and started. */
function AgentIntegrationDiagnosisBody({
	diagnosis,
}: {
	readonly diagnosis: AgentIntegrationDiagnosisView;
}): React.ReactNode {
	return (
		<>
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
			{diagnosis.pending || diagnosis.readOnly === null ? null : (
				<p className={styles['diagnosisText']}>{diagnosis.readOnly}</p>
			)}
		</>
	);
}
