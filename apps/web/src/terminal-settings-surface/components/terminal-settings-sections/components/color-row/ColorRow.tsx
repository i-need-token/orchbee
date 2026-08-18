import { Input } from '@orchbee/ui/components';
import * as React from 'react';

import { FieldControl } from '../field-control/FieldControl';
import { TerminalSettingsRow } from '../../../terminal-settings-row/TerminalSettingsRow';
import styles from './ColorRow.module.css';

interface ColorRowProps {
	readonly color: string;
	readonly description: string;
	readonly disabled: boolean;
	readonly draft: string;
	readonly error: string | null;
	readonly label: string;
	readonly settingId: string;
	readonly visible: boolean;
	readonly onColorChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onDraftChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onDraftKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onInputBlur: React.FocusEventHandler<HTMLInputElement>;
}

export function ColorRow({
	color,
	description,
	disabled,
	draft,
	error,
	label,
	settingId,
	onColorChange,
	onDraftChange,
	onDraftKeyDown,
	onInputBlur,
	visible,
}: ColorRowProps): React.ReactNode {
	return (
		<TerminalSettingsRow
			description={description}
			label={label}
			settingId={settingId}
			visible={visible}
		>
			<FieldControl error={error}>
				<div className={styles['root']}>
					<input
						aria-label={label}
						className={styles['picker']}
						disabled={disabled}
						type='color'
						value={color}
						onChange={onColorChange}
					/>
					<Input
						aria-label={label}
						aria-invalid={error !== null}
						disabled={disabled}
						value={draft}
						onBlur={onInputBlur}
						onChange={onDraftChange}
						onKeyDown={onDraftKeyDown}
					/>
				</div>
			</FieldControl>
		</TerminalSettingsRow>
	);
}
