import { terminalSettingsMessages as messages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalSettingsMachines } from '../../hooks/useTerminalSettingsMachines';
import type {
	TerminalSettingsMachine,
	TerminalSettingsMachines,
} from '../../types/terminal-settings-surface';
import { SettingFailure } from '../setting-failure/SettingFailure';
import { SettingsChildPage } from '../settings-child-page/SettingsChildPage';
import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';
import { MachinePage } from './components/machine-page/MachinePage';
import { MachineStateWord } from './components/machine-state-word/MachineStateWord';
import styles from './MachineSettings.module.css';

interface MachineSettingsProps {
	readonly machines: TerminalSettingsMachines;
	readonly query: string;
}

/**
 * Settings › Machines, as the TUI's: this computer, then each saved machine with its address and
 * either what it has free or its state word; Manage opens the machine's page. Every sentence about a
 * machine comes worded from the machines process, so both Clients say the same thing.
 */
export function MachineSettings({ machines, query }: MachineSettingsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const view = useTerminalSettingsMachines(machines);
	const [shown, setShown] = React.useState<string | null>(null);
	// The machines' free capacity is read only while the list or a machine's page shows it.
	React.useEffect(() => machines.showCapacity(), [machines]);
	if (view === null) return null;
	const searching = query.trim() !== '';
	// The page follows its machine, so a removed machine closes it.
	const machine = searching ? undefined : view.machines.find((entry) => entry.machine === shown);
	const unknown = intl.formatMessage(messages.pageMachinesUnknown);
	const manage = intl.formatMessage(messages.pageMachinesManage);
	return (
		<>
			<SettingsGroup
				description={intl.formatMessage(messages.pageMachinesDescription)}
				note={view.hub === null ? undefined : <span className={styles['note']}>{view.hub}</span>}
			>
				{searching ? null : <SettingFailure message={view.notice?.text ?? null} />}
				<SettingsRow
					description={intl.formatMessage(
						view.machines.length === 0
							? messages.pageMachinesNone
							: messages.pageMachinesColumnNote,
					)}
					label={intl.formatMessage(messages.pageMachinesThisComputer)}
					query={query}
					settingId='machines.local'
				>
					<span className={styles['value']}>{view.local.summary}</span>
				</SettingsRow>
				{view.machines.map((entry) => (
					<SettingsRow
						key={entry.machine}
						description={entry.address ?? unknown}
						label={entry.label}
						query={query}
						settingId={`machines.${entry.machine}`}
					>
						<ListValue machine={entry} />
						<Button
							aria-label={`${manage}: ${entry.label}`}
							size='sm'
							type='button'
							variant='outline'
							onClick={() => setShown(entry.machine)}
						>
							{manage}
						</Button>
					</SettingsRow>
				))}
			</SettingsGroup>
			{machine === undefined ? null : (
				<SettingsChildPage title={machine.label} onBack={() => setShown(null)}>
					<MachinePage machine={machine} machines={machines} passwords={view.passwords} />
				</SettingsChildPage>
			)}
		</>
	);
}

/** What a connected machine has free, or else its state word in its tone. */
function ListValue({ machine }: { readonly machine: TerminalSettingsMachine }): React.ReactNode {
	if (machine.state === 'online' && !machine.stopping) {
		return <span className={styles['value']}>{machine.capacity.summary}</span>;
	}
	return <MachineStateWord machine={machine} />;
}
