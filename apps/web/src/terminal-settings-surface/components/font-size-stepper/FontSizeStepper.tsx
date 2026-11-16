import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	InputGroup,
	InputGroupAddon,
	InputGroupButton,
	InputGroupInput,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import styles from './FontSizeStepper.module.css';

interface FontSizeStepperProps {
	/** The typed value; it commits on Enter or when the field loses focus. */
	readonly draft: string;
	readonly invalid: boolean;
	readonly label: string;
	readonly maximum: number;
	readonly minimum: number;
	readonly readOnly: boolean;
	/** The stored size, which the buttons step from. */
	readonly value: number;
	readonly onBlur: () => void;
	readonly onChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onStep: (size: number) => void;
}

/**
 * A size as minus, the value, and plus. Each press, and the Up and Down arrows in the field, moves
 * one step and applies at once; a typed value applies on Enter or when the field loses focus.
 */
export function FontSizeStepper({
	draft,
	invalid,
	label,
	maximum,
	minimum,
	readOnly,
	value,
	onBlur,
	onChange,
	onKeyDown,
	onStep,
}: FontSizeStepperProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const step = (direction: -1 | 1): void => {
		const next = Math.min(maximum, Math.max(minimum, value + direction));
		if (next !== value) onStep(next);
	};
	return (
		<InputGroup className={styles['stepper']} data-disabled={readOnly || undefined}>
			<InputGroupAddon>
				<InputGroupButton
					aria-label={intl.formatMessage(terminalSettingsMessages.pageFontSizeDecrease)}
					disabled={readOnly || value <= minimum}
					size='icon-xs'
					onClick={(): void => {
						step(-1);
					}}
				>
					<ProductIcon name='remove' />
				</InputGroupButton>
			</InputGroupAddon>
			<InputGroupInput
				aria-invalid={invalid || undefined}
				aria-label={label}
				aria-valuemax={maximum}
				aria-valuemin={minimum}
				aria-valuenow={value}
				className={styles['value']}
				inputMode='numeric'
				readOnly={readOnly}
				role='spinbutton'
				spellCheck={false}
				value={draft}
				onBlur={onBlur}
				onChange={onChange}
				onKeyDown={(event): void => {
					if (!readOnly && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
						event.preventDefault();
						step(event.key === 'ArrowUp' ? 1 : -1);
						return;
					}
					onKeyDown(event);
				}}
			/>
			<InputGroupAddon align='inline-end'>
				<InputGroupButton
					aria-label={intl.formatMessage(terminalSettingsMessages.pageFontSizeIncrease)}
					disabled={readOnly || value >= maximum}
					size='icon-xs'
					onClick={(): void => {
						step(1);
					}}
				>
					<ProductIcon name='add' />
				</InputGroupButton>
			</InputGroupAddon>
		</InputGroup>
	);
}
