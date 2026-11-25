import { terminalSettingsMessages as messages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button, Input } from '@orchbee/ui/components';
import * as React from 'react';

import type {
	TerminalSettingsMachine,
	TerminalSettingsMachines,
} from '../../../../types/terminal-settings-surface';
import { SettingsGroup } from '../../../settings-group/SettingsGroup';
import { SettingsRow } from '../../../settings-row/SettingsRow';
import { MachineColorSelect } from '../machine-color-select/MachineColorSelect';
import { MachineStateWord } from '../machine-state-word/MachineStateWord';
import styles from './MachinePage.module.css';

/** A machine's name holds at most this many UTF-8 bytes, as the catalog admits. */
const LABEL_BYTES = 128;

interface MachinePageProps {
	readonly machine: TerminalSettingsMachine;
	readonly machines: TerminalSettingsMachines;
	readonly passwords: boolean;
}

/**
 * A saved machine's page, as the TUI's: its name, color, address, status with its one action, what
 * it has free while connected, its saved password, then stopping its terminals and removing it.
 * Nothing changes on a machine whose terminals are being stopped.
 */
export function MachinePage({ machine, machines, passwords }: MachinePageProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const idle = !machine.stopping;
	const id = machine.machine;
	return (
		<>
			<SettingsGroup>
				<NameRow machine={machine} machines={machines} />
				<SettingsRow
					description={intl.formatMessage(messages.pageMachinesColorDescription)}
					label={intl.formatMessage(messages.pageMachinesColorLabel)}
					query=''
				>
					<MachineColorSelect
						named
						color={machine.color}
						disabled={!idle}
						onChoose={(color) => machines.request({ act: 'set-color', color, machine: id })}
					/>
				</SettingsRow>
				<SettingsRow
					description={intl.formatMessage(messages.pageMachinesAddressDescription)}
					label={intl.formatMessage(messages.pageMachinesAddressLabel)}
					query=''
				>
					<span className={styles['value']}>
						{machine.address ?? intl.formatMessage(messages.pageMachinesUnknown)}
					</span>
				</SettingsRow>
				<SettingsRow
					description={machine.status}
					label={intl.formatMessage(messages.pageMachinesStatusLabel)}
					query=''
				>
					<MachineStateWord machine={machine} />
					{machine.action === null ? null : (
						<Button
							size='sm'
							type='button'
							variant='outline'
							onClick={() => {
								if (machine.action !== null)
									machines.request({ act: machine.action.act, machine: id });
							}}
						>
							{machine.action.label}
						</Button>
					)}
				</SettingsRow>
				{machine.state === 'online' ? (
					<SettingsRow
						description={machine.capacity.long}
						label={intl.formatMessage(messages.pageMachinesFreeLabel)}
						query=''
					/>
				) : null}
				{passwords ? (
					<SettingsRow
						description={intl.formatMessage(messages.pageMachinesPasswordDescription)}
						label={intl.formatMessage(messages.pageMachinesPasswordLabel)}
						query=''
					>
						<Button
							disabled={!idle}
							size='sm'
							type='button'
							variant='outline'
							onClick={() => machines.request({ act: 'forget-password', machine: id })}
						>
							{intl.formatMessage(messages.pageMachinesPasswordForget)}
						</Button>
					</SettingsRow>
				) : null}
			</SettingsGroup>
			<SettingsGroup>
				<StopRow machine={machine} machines={machines} />
				<SettingsRow
					description={machine.removeDescription}
					label={intl.formatMessage(messages.pageMachinesRemoveLabel)}
					query=''
				>
					<Button
						disabled={!idle}
						size='sm'
						type='button'
						variant='destructive'
						onClick={() => machines.request({ act: 'remove', machine: id })}
					>
						{intl.formatMessage(messages.pageMachinesRemoveAction)}
					</Button>
				</SettingsRow>
			</SettingsGroup>
		</>
	);
}

/** The name field: Enter or leaving it renames the machine; Escape puts the saved name back. */
function NameRow({ machine, machines }: Omit<MachinePageProps, 'passwords'>): React.ReactNode {
	const intl = useOrchbeeIntl();
	const label = intl.formatMessage(messages.pageMachinesNameLabel);
	const [draft, setDraft] = React.useState(machine.label);
	const [saved, setSaved] = React.useState(machine.label);
	// A rename from anywhere, this page's included, replaces the field once it is reported.
	if (saved !== machine.label) {
		setSaved(machine.label);
		setDraft(machine.label);
	}
	const commit = (): void => {
		const next = draft.trim();
		if (next === '' || next === machine.label) {
			setDraft(machine.label);
			return;
		}
		machines.request({ act: 'rename', label: next, machine: machine.machine });
	};
	return (
		<SettingsRow
			description={intl.formatMessage(messages.pageMachinesNameDescription)}
			label={label}
			query=''
		>
			<Input
				aria-label={label}
				className={styles['name']}
				disabled={machine.stopping}
				spellCheck={false}
				value={draft}
				onBlur={commit}
				onChange={(event) => {
					const next = event.currentTarget.value;
					if (new TextEncoder().encode(next).length <= LABEL_BYTES) setDraft(next);
				}}
				onKeyDown={(event) => {
					if (event.nativeEvent.isComposing) return;
					if (event.key === 'Enter') {
						event.preventDefault();
						commit();
					} else if (event.key === 'Escape' && draft !== machine.label) {
						// The first Escape gives the saved name back; the next leaves the page.
						event.stopPropagation();
						setDraft(machine.label);
					}
				}}
			/>
		</SettingsRow>
	);
}

/**
 * Stop its terminals: the first press states what stops, and Stop there stops it. The row says
 * so while the machine is not connected, and while its terminals are being stopped.
 */
function StopRow({ machine, machines }: Omit<MachinePageProps, 'passwords'>): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [confirming, setConfirming] = React.useState(false);
	const online = machine.state === 'online';
	const available = online && !machine.stopping;
	if (confirming && !available) setConfirming(false);
	let description = intl.formatMessage(messages.pageMachinesStopDescription);
	let tone: 'neutral' | 'warning' = 'neutral';
	if (confirming) {
		description = machine.stopConfirmation;
		tone = 'warning';
	} else if (machine.stopping) {
		description = intl.formatMessage(messages.pageMachinesStopping);
	} else if (!online) {
		description = machine.notConnected;
	}
	const stop = intl.formatMessage(messages.pageMachinesStopAction);
	return (
		<SettingsRow
			description={description}
			descriptionTone={tone}
			label={intl.formatMessage(messages.pageMachinesStopLabel)}
			query=''
		>
			{confirming ? (
				<span className={styles['confirm']}>
					<Button size='sm' type='button' variant='ghost' onClick={() => setConfirming(false)}>
						{intl.formatMessage(messages.pageMachinesStopCancel)}
					</Button>
					<Button
						size='sm'
						type='button'
						variant='destructive'
						onClick={() => {
							setConfirming(false);
							machines.request({ act: 'stop', machine: machine.machine });
						}}
					>
						{stop}
					</Button>
				</span>
			) : (
				<Button
					disabled={!available}
					size='sm'
					type='button'
					variant='destructive'
					onClick={() => setConfirming(true)}
				>
					{stop}
				</Button>
			)}
		</SettingsRow>
	);
}
