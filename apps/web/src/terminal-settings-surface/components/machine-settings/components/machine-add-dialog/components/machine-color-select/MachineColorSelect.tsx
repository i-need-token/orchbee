import { terminalSettingsMessages as messages, useOrchbeeIntl } from '@orchbee/i18n';
import { Select, SelectContent, SelectItem, SelectTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalSettingsMachineColor } from '../../../../../../types/terminal-settings-surface';
import { MACHINE_COLORS } from '../../../../constants/machine-colors';
import styles from './MachineColorSelect.module.css';

interface MachineColorSelectProps {
	readonly color: TerminalSettingsMachineColor;
	readonly disabled: boolean;
	readonly onChoose: (color: TerminalSettingsMachineColor) => void;
}

/**
 * A new machine's color before its address, as the TUI's form shows it: only the chosen color's
 * dot, which opens the colors with their names; choosing one closes the list.
 */
export function MachineColorSelect({
	color,
	disabled,
	onChoose,
}: MachineColorSelectProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const colors = MACHINE_COLORS.map(([value, name]) => ({
		label: intl.formatMessage(name),
		value,
	}));
	const chosen = colors.find((entry) => entry.value === color) ?? colors[0];
	return (
		<Select
			disabled={disabled}
			items={colors}
			value={color}
			onValueChange={(next): void => {
				if (next !== null && next !== color) onChoose(next);
			}}
		>
			{/* The dot alone shows the color; the control's name says which. */}
			<SelectTrigger
				aria-label={intl.formatMessage(messages.pageMachinesColorChosen, {
					color: chosen?.label ?? color,
				})}
				className={styles['trigger']}
			>
				<span aria-hidden='true' className={styles['dot']} data-machine-color={color} />
			</SelectTrigger>
			<SelectContent align='start'>
				{colors.map((entry) => (
					<SelectItem key={entry.value} value={entry.value}>
						<span aria-hidden='true' className={styles['dot']} data-machine-color={entry.value} />
						{entry.label}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	);
}
