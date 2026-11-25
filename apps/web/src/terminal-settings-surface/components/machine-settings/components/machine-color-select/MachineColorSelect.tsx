import { terminalSettingsMessages as messages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input, Select, SelectContent, SelectItem, SelectTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import {
	MACHINE_COLOR_PRESETS,
	machineColorValue,
	parseMachineCustomColor,
} from '../../../../../machine-color';
import type { MachineColorPreset } from '../../../../../machine-color';
import type { TerminalSettingsMachineColor } from '../../../../types/terminal-settings-surface';
import { MACHINE_COLOR_NAMES } from '../../constants/machine-colors';
import styles from './MachineColorSelect.module.css';

/** The list's last entry, which asks for a color of the person's own. */
const CUSTOM = 'custom';

/** An entry of the list: a preset, or Custom…. */
type Entry = MachineColorPreset | typeof CUSTOM;

/** The native color picker's color when neither the typed text nor a shown color gives one. */
const PICKER_FALLBACK = '#000000';

/**
 * The `#rrggbb` an element's background is shown in, so the system color picker opens on the
 * color the machine has: a preset's theme token may be written in any CSS color space.
 */
function shownColor(element: HTMLElement | null): string | null {
	const context = element === null ? null : document.createElement('canvas').getContext('2d');
	if (element === null || context === null) return null;
	context.fillStyle = getComputedStyle(element).backgroundColor;
	context.fillRect(0, 0, 1, 1);
	const [red = 0, green = 0, blue = 0] = context.getImageData(0, 0, 1, 1).data;
	return `#${[red, green, blue].map((channel) => channel.toString(16).padStart(2, '0')).join('')}`;
}

interface MachineColorSelectProps {
	readonly color: TerminalSettingsMachineColor;
	readonly disabled: boolean;
	/** The chosen color's name beside its dot, where no other control names it. */
	readonly named?: boolean | undefined;
	readonly onChoose: (color: TerminalSettingsMachineColor) => void;
}

/**
 * A machine's color, as the TUI chooses it: the chosen color's dot opens the presets with their
 * names, then Custom…. A custom color, or Custom… chosen, shows the color's own field after the
 * dot: the system's color picker and the typed `#rrggbb`, taken on Enter or when it is left.
 */
export function MachineColorSelect({
	color,
	disabled,
	named = false,
	onChoose,
}: MachineColorSelectProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const preset = MACHINE_COLOR_PRESETS.find((entry) => entry === color);
	const custom = preset === undefined;
	// Custom… chosen on a preset's machine, before any color is typed.
	const [asking, setAsking] = React.useState(false);
	// The typed text while it differs from the color; `null` shows the color itself.
	const [typed, setTyped] = React.useState<string | null>(null);
	const [invalid, setInvalid] = React.useState(false);
	// The preset's shown color, where the system color picker opens after Custom….
	const [pickerStart, setPickerStart] = React.useState<string | null>(null);
	const dot = React.useRef<HTMLSpanElement>(null);
	const hex = React.useRef<HTMLInputElement>(null);
	// Custom… was just chosen: the list hands focus to the typed color when it closes.
	const focusHex = React.useRef(false);
	const presets = MACHINE_COLOR_NAMES.map(([value, name]) => ({
		label: intl.formatMessage(name),
		value,
	}));
	const customLabel = intl.formatMessage(messages.pageMachinesColorCustom);
	const items: { readonly label: string; readonly value: Entry }[] = [
		...presets,
		{ label: customLabel, value: CUSTOM },
	];
	// A custom color reads "Custom" beside its dot, since its own field shows the value; the
	// control's accessible name still says the value.
	const chosenName = custom
		? color
		: (presets.find((entry) => entry.value === color)?.label ?? color);
	const shownName = custom ? intl.formatMessage(messages.pageMachinesColorCustomName) : chosenName;
	const text = typed ?? (custom ? color : '');
	const take = (next: TerminalSettingsMachineColor): void => {
		setTyped(null);
		setInvalid(false);
		if (next !== color) onChoose(next);
	};
	const submit = (): void => {
		if (typed === null) return;
		const next = parseMachineCustomColor(typed);
		if (next === null) setInvalid(true);
		else take(next);
	};
	return (
		<span className={styles['root']}>
			<Select<Entry>
				disabled={disabled}
				items={items}
				value={asking || preset === undefined ? CUSTOM : preset}
				onValueChange={(next): void => {
					if (next === null) return;
					if (next === CUSTOM) {
						if (!custom) setPickerStart(shownColor(dot.current));
						focusHex.current = true;
						setAsking(true);
						return;
					}
					setAsking(false);
					take(next);
				}}
			>
				{/* The dot shows the color; the control's name says which. */}
				<SelectTrigger
					aria-label={intl.formatMessage(messages.pageMachinesColorChosen, { color: chosenName })}
					className={styles['trigger']}
				>
					<span
						ref={dot}
						aria-hidden='true'
						className={styles['dot']}
						style={{ '--ob-comp-machine-color': machineColorValue(color) }}
					/>
					{named ? <span aria-hidden='true'>{shownName}</span> : null}
				</SelectTrigger>
				<SelectContent
					align='start'
					finalFocus={(): HTMLElement | true => {
						const toHex = focusHex.current;
						focusHex.current = false;
						return (toHex ? hex.current : null) ?? true;
					}}
				>
					{presets.map((entry) => (
						<SelectItem key={entry.value} value={entry.value}>
							<span
								aria-hidden='true'
								className={styles['dot']}
								style={{ '--ob-comp-machine-color': machineColorValue(entry.value) }}
							/>
							{entry.label}
						</SelectItem>
					))}
					<SelectItem value={CUSTOM}>
						{custom ? (
							<span
								aria-hidden='true'
								className={styles['dot']}
								style={{ '--ob-comp-machine-color': machineColorValue(color) }}
							/>
						) : (
							<span aria-hidden='true' className={styles['ring']} />
						)}
						{customLabel}
					</SelectItem>
				</SelectContent>
			</Select>
			{custom || asking ? (
				<span className={styles['custom']}>
					<input
						aria-label={intl.formatMessage(messages.pageMachinesColorCustomLabel)}
						className={styles['picker']}
						disabled={disabled}
						type='color'
						value={
							parseMachineCustomColor(text) ??
							parseMachineCustomColor(color) ??
							pickerStart ??
							PICKER_FALLBACK
						}
						onChange={(event) => {
							const next = parseMachineCustomColor(event.currentTarget.value);
							if (next !== null) take(next);
						}}
					/>
					<Input
						ref={hex}
						aria-invalid={invalid}
						aria-label={intl.formatMessage(messages.pageMachinesColorCustomLabel)}
						className={styles['hex']}
						disabled={disabled}
						maxLength={7}
						placeholder={intl.formatMessage(messages.pageMachinesColorCustomPlaceholder)}
						spellCheck={false}
						value={text}
						onBlur={submit}
						onChange={(event) => {
							setTyped(event.currentTarget.value);
							setInvalid(false);
						}}
						onKeyDown={(event) => {
							if (event.key !== 'Enter') return;
							// Enter takes the typed color, not the form it sits in.
							event.preventDefault();
							submit();
						}}
					/>
				</span>
			) : null}
		</span>
	);
}
