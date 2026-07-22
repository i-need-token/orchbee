import {
	ContextMenuGroup,
	ContextMenuLabel,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchTabColor } from '../../../../types/workbench-tab';
import styles from './WorkbenchColorPalette.module.css';

export interface WorkbenchColorChoice {
	readonly label: string;
	readonly value: WorkbenchTabColor;
}

interface WorkbenchColorPaletteProps {
	readonly color: WorkbenchTabColor;
	readonly colorChoices: readonly WorkbenchColorChoice[];
	readonly label: string;
	readonly onColorChange: ((color: WorkbenchTabColor) => void) | undefined;
}

export function WorkbenchColorPalette({
	color,
	colorChoices,
	label,
	onColorChange,
}: WorkbenchColorPaletteProps): React.ReactNode {
	return (
		<ContextMenuGroup>
			<ContextMenuLabel className={styles['label']}>{label}</ContextMenuLabel>
			<ContextMenuRadioGroup aria-label={label} className={styles['palette']} value={color}>
				{colorChoices.map((choice) => (
					<ContextMenuRadioItem
						key={choice.value}
						aria-label={choice.label}
						className={styles['choice']}
						closeOnClick
						data-color={choice.value}
						disabled={onColorChange === undefined}
						value={choice.value}
						onClick={() => {
							onColorChange?.(choice.value);
						}}
					>
						<span className={styles['swatch']} data-color={choice.value} />
					</ContextMenuRadioItem>
				))}
			</ContextMenuRadioGroup>
		</ContextMenuGroup>
	);
}
