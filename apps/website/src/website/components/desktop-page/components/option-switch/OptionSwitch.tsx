import { useId, type ReactElement, type ReactNode } from 'react';

import styles from './OptionSwitch.module.css';

/** One choice of an OptionSwitch. */
export interface SwitchOption {
	readonly id: string;
	readonly label: ReactNode;
}

/**
 * A labelled row of buttons of which one is pressed, under the figure it controls. `name` marks
 * the row for its activation (`data-option-switch`), which shows it and binds it with
 * bindOptionSwitch; without script the row stays hidden.
 */
export function OptionSwitch({
	name,
	label,
	options,
	pressed,
}: {
	name: string;
	label: ReactNode;
	options: readonly SwitchOption[];
	pressed: string;
}): ReactElement {
	const labelId = useId();
	return (
		<div className={styles['row']} data-option-switch={name} hidden>
			<span id={labelId}>{label}</span>
			<div className={styles['switch']} role='group' aria-labelledby={labelId}>
				{options.map((option) => (
					<button
						key={option.id}
						type='button'
						data-option={option.id}
						aria-pressed={option.id === pressed}
					>
						{option.label}
					</button>
				))}
			</div>
		</div>
	);
}
