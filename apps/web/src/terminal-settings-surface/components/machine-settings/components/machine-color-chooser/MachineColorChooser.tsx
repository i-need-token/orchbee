import { useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalSettingsMachineColor } from '../../../../types/terminal-settings-surface';
import { MACHINE_COLORS } from '../../constants/machine-colors';
import styles from './MachineColorChooser.module.css';

/** How far each arrow key moves the choice along the swatches, wrapping at either end. */
const STEPS: Readonly<Partial<Record<string, number>>> = {
	ArrowDown: 1,
	ArrowLeft: -1,
	ArrowRight: 1,
	ArrowUp: -1,
};

interface MachineColorChooserProps {
	readonly color: TerminalSettingsMachineColor;
	readonly disabled?: boolean | undefined;
	/** The group's accessible name. */
	readonly label: string;
	readonly onChoose: (color: TerminalSettingsMachineColor) => void;
}

/**
 * A machine's color: four swatches, the chosen one ringed and checked, and its name beside them in
 * that color. One radio group, so Tab reaches the chosen swatch and the arrow keys choose another.
 */
export function MachineColorChooser({
	color,
	disabled = false,
	label,
	onChoose,
}: MachineColorChooserProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const swatches = React.useRef<(HTMLButtonElement | null)[]>([]);
	const [, chosen] = MACHINE_COLORS.find(([value]) => value === color) ?? MACHINE_COLORS[0];
	const move = (from: number, step: number): void => {
		const at = (from + step + MACHINE_COLORS.length) % MACHINE_COLORS.length;
		const entry = MACHINE_COLORS[at];
		if (entry === undefined) return;
		swatches.current[at]?.focus();
		onChoose(entry[0]);
	};
	return (
		<div aria-label={label} className={styles['root']} role='radiogroup'>
			{MACHINE_COLORS.map(([value, name], index) => {
				const checked = value === color;
				return (
					<button
						key={value}
						ref={(element) => {
							swatches.current[index] = element;
						}}
						aria-checked={checked}
						aria-label={intl.formatMessage(name)}
						className={styles['swatch']}
						data-machine-color={value}
						disabled={disabled}
						role='radio'
						tabIndex={checked ? 0 : -1}
						type='button'
						onClick={() => {
							if (!checked) onChoose(value);
						}}
						onKeyDown={(event) => {
							const step = STEPS[event.key];
							if (step === undefined) return;
							event.preventDefault();
							move(index, step);
						}}
					>
						{checked ? <ProductIcon name='check' /> : null}
					</button>
				);
			})}
			<span aria-hidden='true' className={styles['name']} data-machine-color={color}>
				{intl.formatMessage(chosen)}
			</span>
		</div>
	);
}
