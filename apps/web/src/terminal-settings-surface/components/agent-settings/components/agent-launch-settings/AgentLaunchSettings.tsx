import {
	terminalSettingsMessages as messages,
	useOrchbeeIntl,
	type MessageDescriptor,
} from '@orchbee/i18n';
import { Button, Input, ProductIcon, Switch } from '@orchbee/ui/components';
import {
	AGENT_YOLO_PROVIDERS,
	agentArgumentsKey,
	agentOverridesYolo,
	agentYoloFlag,
	agentYoloKey,
	parseAgentArguments,
	type AgentYoloProvider,
	type WorkbenchAgentCatalog,
} from '@orchbee/workbench-client-protocol';
import { useId, useState, useSyncExternalStore, type ReactNode } from 'react';

import { AgentProviderIcon } from '../../../../../agent-provider-icon';
import { readSetting, type UserSettingsControl } from '../../../../../user-settings';
import { useSetting } from '../../../../hooks/useSetting';
import { useSettingDraft } from '../../../../hooks/useSettingDraft';
import { useUserSettings } from '../../../../hooks/useUserSettings';
import type { AgentLaunchCatalogSource } from '../../../../types/terminal-settings-surface';
import { matchesSettingsQuery } from '../../../../utils/matchesSettingsQuery';
import { SettingFailure } from '../../../setting-failure/SettingFailure';
import { SettingsChildPage } from '../../../settings-child-page/SettingsChildPage';
import { SettingsGroup } from '../../../settings-group/SettingsGroup';
import { SettingsListFilter } from '../../../settings-list-filter/SettingsListFilter';
import { SettingsRow } from '../../../settings-row/SettingsRow';
import styles from './AgentLaunchSettings.module.css';

type Agent = WorkbenchAgentCatalog['agents'][number];

const subscribeToNothing = () => () => undefined;
const yoloDescriptions: Readonly<Partial<Record<AgentYoloProvider, MessageDescriptor>>> = {
	'claude-code': messages.pageAgentLaunchYoloClaude,
	codex: messages.pageAgentLaunchYoloCodex,
	gemini: messages.pageAgentLaunchYoloGemini,
} as const;

interface AgentLaunchSettingsProps {
	readonly catalog?: AgentLaunchCatalogSource | undefined;
	readonly query: string;
	readonly userSettings: UserSettingsControl;
}

/** The provider catalog and one immediately committed editor per Agent. */
export function AgentLaunchSettings({
	catalog,
	query,
	userSettings,
}: AgentLaunchSettingsProps): ReactNode {
	const intl = useOrchbeeIntl();
	const icons = useId();
	const read = () => catalog?.getSnapshot() ?? null;
	const agents = useSyncExternalStore(catalog?.subscribe ?? subscribeToNothing, read, read);
	const { snapshot } = useUserSettings(userSettings);
	const [filter, setFilter] = useState('');
	const [selected, setSelected] = useState<Agent | null>(null);
	const groupLabel = intl.formatMessage(messages.pageAgentLaunchGroup);
	const description = intl.formatMessage(messages.pageAgentLaunchDescription);
	const offered = (agents?.agents ?? [])
		.filter((agent) => snapshot?.values[agentArgumentsKey(agent.providerId)] !== undefined)
		.sort((left, right) => left.productName.localeCompare(right.productName, 'en'));
	const matching = offered.filter((agent) =>
		matchesSettingsQuery(
			query,
			[
				agent.productName,
				agent.command,
				groupLabel,
				intl.formatMessage(messages.pageAgentLaunchYolo),
				intl.formatMessage(messages.pageAgentLaunchArguments),
			],
			intl.locale,
		),
	);
	const canFilter = matching.length > 8;
	const shown = canFilter
		? matching.filter((agent) =>
				matchesSettingsQuery(filter, [agent.productName, agent.command], intl.locale),
			)
		: matching;
	if (matching.length === 0) return null;
	return (
		<>
			<SettingsGroup description={description} label={groupLabel}>
				{canFilter ? (
					<SettingsListFilter
						onChange={setFilter}
						placeholder={intl.formatMessage(messages.pageAgentLaunchFilter)}
						shown={shown.length}
						total={matching.length}
						value={filter}
					/>
				) : null}
				{shown.map((agent) => (
					<Button
						className={styles['launchEntry']}
						data-settings-row
						key={agent.providerId}
						type='button'
						variant='ghost'
						onClick={() => setSelected(agent)}
					>
						<AgentProviderIcon
							agents={[agent.providerId]}
							gradientPrefix={`${icons}-${agent.providerId}`}
						/>
						<span>{agent.productName}</span>
						<code>{agent.command}</code>
						<ProductIcon name='chevron-right' size='disclosure' />
					</Button>
				))}
			</SettingsGroup>
			{selected === null ? null : (
				<SettingsChildPage title={selected.productName} onBack={() => setSelected(null)}>
					<AgentLaunchEditor
						key={selected.providerId}
						agent={selected}
						userSettings={userSettings}
					/>
				</SettingsChildPage>
			)}
		</>
	);
}

function AgentLaunchEditor({
	agent,
	userSettings,
}: {
	readonly agent: Agent;
	readonly userSettings: UserSettingsControl;
}): ReactNode {
	const intl = useOrchbeeIntl();
	const settings = useUserSettings(userSettings);
	const key = agentArgumentsKey(agent.providerId);
	const argumentsSetting = useSetting(userSettings, key);
	const { draft, change } = useSettingDraft(argumentsSetting.value, argumentsSetting.failure);
	const [invalid, setInvalid] = useState<'unclosed-quote' | 'invalid-value' | null>(null);
	const label = intl.formatMessage(messages.pageAgentLaunchArguments);
	const yolo = AGENT_YOLO_PROVIDERS.find((provider) => provider === agent.providerId);
	const saved = parseAgentArguments(argumentsSetting.value);
	const overridden = yolo !== undefined && saved.valid && agentOverridesYolo(yolo, saved.words);
	const yoloEnabled =
		yolo !== undefined && !overridden && readSetting(settings.snapshot, agentYoloKey(yolo));
	const command = [agent.command, yoloEnabled ? agentYoloFlag(yolo) : '', argumentsSetting.value]
		.filter((part) => part !== '')
		.join(' ');
	const readOnly = argumentsSetting.readOnly || !settings.available;
	const commit = (): void => {
		const next = parseAgentArguments(draft);
		if (!next.valid) {
			setInvalid(next.reason === 'unclosed-quote' ? 'unclosed-quote' : 'invalid-value');
			return;
		}
		setInvalid(null);
		const value = next.words.length === 0 ? '' : draft;
		if (!readOnly && value !== argumentsSetting.value) argumentsSetting.set(value);
	};
	const failure =
		invalid === null
			? argumentsSetting.failureMessage
			: intl.formatMessage(
					invalid === 'unclosed-quote'
						? messages.pageAgentLaunchUnclosedQuote
						: messages.pageAgentLaunchInvalidArguments,
				);
	return (
		<SettingsGroup
			description={intl.formatMessage(messages.pageAgentLaunchDescription)}
			label={intl.formatMessage(messages.pageAgentLaunchGroup)}
		>
			{yolo === undefined || settings.snapshot?.values[agentYoloKey(yolo)] === undefined ? null : (
				<AgentYoloSetting overridden={overridden} provider={yolo} userSettings={userSettings} />
			)}
			<SettingsRow
				description={intl.formatMessage(messages.pageAgentLaunchArgumentsDescription)}
				label={label}
				modified={argumentsSetting.modified}
				onRestore={
					readOnly
						? undefined
						: () => {
								setInvalid(null);
								change('');
								argumentsSetting.restore();
							}
				}
				preview={
					<div className={styles['launchPreview']}>
						<span>{intl.formatMessage(messages.pageAgentLaunchPreview)}</span>
						<output>{command}</output>
					</div>
				}
				query=''
				settingId={key}
				status={<SettingFailure message={failure} />}
			>
				<Input
					aria-invalid={invalid === null ? undefined : true}
					aria-label={label}
					className={styles['launchArguments']}
					readOnly={readOnly}
					spellCheck={false}
					value={draft}
					onBlur={commit}
					onChange={(event) => {
						change(event.currentTarget.value);
						setInvalid(null);
					}}
					onKeyDown={(event) => {
						if (event.nativeEvent.isComposing) return;
						if (event.key === 'Enter') {
							event.preventDefault();
							commit();
						}
					}}
				/>
			</SettingsRow>
		</SettingsGroup>
	);
}

function AgentYoloSetting({
	overridden,
	provider,
	userSettings,
}: {
	readonly overridden: boolean;
	readonly provider: AgentYoloProvider;
	readonly userSettings: UserSettingsControl;
}): ReactNode {
	const intl = useOrchbeeIntl();
	const key = agentYoloKey(provider);
	const setting = useSetting(userSettings, key);
	const { available } = useUserSettings(userSettings);
	const label = intl.formatMessage(messages.pageAgentLaunchYolo);
	return (
		<SettingsRow
			description={intl.formatMessage(
				overridden
					? messages.pageAgentLaunchYoloOverride
					: (yoloDescriptions[provider] ?? messages.pageAgentLaunchYoloDescription),
			)}
			label={label}
			modified={setting.modified}
			onRestore={setting.readOnly || !available ? undefined : setting.restore}
			query=''
			settingId={key}
			status={<SettingFailure message={setting.failureMessage} />}
		>
			<Switch
				aria-label={label}
				checked={!overridden && setting.value}
				disabled={overridden || setting.readOnly || !available}
				onCheckedChange={setting.set}
			/>
		</SettingsRow>
	);
}
