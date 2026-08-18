import { Button, Input, NativeSelect, NativeSelectOption, Switch } from '@orchbee/ui/components';
import * as React from 'react';

import type { PlannedTerminalSettingControlKind } from '../../../../types/terminal-settings-surface';
import styles from './PlannedControl.module.css';

interface PlannedControlProps {
	readonly defaultValue: string;
	readonly kind: PlannedTerminalSettingControlKind;
	readonly label: string;
	readonly manage: string;
}

export function PlannedControl({
	defaultValue,
	kind,
	label,
	manage,
}: PlannedControlProps): React.ReactNode {
	switch (kind) {
		case 'action':
			return (
				<Button className={styles['button']} disabled variant='outline'>
					{manage}
				</Button>
			);
		case 'color':
			return (
				<div className={styles['colorControl']}>
					<input
						aria-label={label}
						className={styles['swatch']}
						defaultValue='#808080'
						disabled
						type='color'
					/>
					<span>{defaultValue}</span>
				</div>
			);
		case 'number':
			return <Input aria-label={label} className={styles['input']} disabled value={defaultValue} />;
		case 'select':
			return (
				<NativeSelect aria-label={label} className={styles['select']} disabled value='default'>
					<NativeSelectOption value='default'>{defaultValue}</NativeSelectOption>
				</NativeSelect>
			);
		case 'text':
			return <Input aria-label={label} className={styles['input']} disabled value={defaultValue} />;
		case 'toggle':
			return <Switch aria-label={label} disabled />;
	}
}
