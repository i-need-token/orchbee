import { useId, type ReactElement, type ReactNode } from 'react';

import type { TerminalThemeData } from '../../../terminal-text/terminalThemeData';
import styles from './OptionSwitch.module.css';

/** One choice of an OptionSwitch; `theme` is a terminal theme its button carries as page data. */
export interface SwitchOption {
	readonly id: string;
	readonly label: ReactNode;
	readonly theme?: TerminalThemeData;
}

/**
 * A labelled row of buttons of which one is pressed, with an optional note at the row's end.
 * `name` marks the row for its activation (`data-option-switch`), which shows it and binds it with
 * bindOptionSwitch; without script the row stays hidden. `placement` sets the room the row keeps
 * toward the figure it controls: `above` or `below` it.
 */
export function OptionSwitch({
	name,
	label,
	options,
	pressed,
	note,
	placement,
}: {
	name: string;
	label: ReactNode;
	options: readonly SwitchOption[];
	pressed: string;
	note?: ReactNode;
	placement: 'above' | 'below';
}): ReactElement {
	const labelId = useId();
	return (
		<div
			className={placement === 'above' ? styles['above'] : styles['below']}
			data-option-switch={name}
			hidden
		>
			<span id={labelId}>{label}</span>
			<div className={styles['switch']} role='group' aria-labelledby={labelId}>
				{options.map((option) => (
					<button
						key={option.id}
						type='button'
						data-option={option.id}
						data-terminal-theme={option.theme?.polarity}
						data-terminal-colors={option.theme?.colors}
						aria-pressed={option.id === pressed}
					>
						{option.label}
					</button>
				))}
			</div>
			{note === undefined ? null : <span className={styles['note']}>{note}</span>}
		</div>
	);
}
