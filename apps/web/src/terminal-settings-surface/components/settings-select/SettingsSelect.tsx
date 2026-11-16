import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from '@orchbee/ui/components';
import * as React from 'react';

interface SettingsSelectOption<Value extends string> {
	readonly label: string;
	readonly value: Value;
}

interface SettingsSelectProps<Value extends string> {
	readonly disabled?: boolean | undefined;
	readonly id?: string | undefined;
	/** The accessible name; omitted only when a visible label names the control by `id`. */
	readonly label?: string | undefined;
	readonly options: readonly SettingsSelectOption<Value>[];
	readonly value: Value;
	readonly onValueChange: (value: Value) => void;
}

/**
 * A setting's finite choice: the shared select with the row's compact trigger. Choosing the entry
 * already shown reports nothing, as a native select would not.
 */
export function SettingsSelect<Value extends string>({
	disabled = false,
	id,
	label,
	options,
	value,
	onValueChange,
}: SettingsSelectProps<Value>): React.ReactNode {
	return (
		<Select
			disabled={disabled}
			items={options}
			value={value}
			onValueChange={(next): void => {
				if (next !== null && next !== value) onValueChange(next);
			}}
		>
			<SelectTrigger aria-label={label} id={id} size='sm'>
				<SelectValue />
			</SelectTrigger>
			<SelectContent>
				{options.map((option) => (
					<SelectItem key={option.value} value={option.value}>
						{option.label}
					</SelectItem>
				))}
			</SelectContent>
		</Select>
	);
}
