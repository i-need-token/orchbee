import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchColumnToggle.module.css';

interface WorkbenchColumnToggleProps {
	/** A count badge over the icon's bottom end, which `descriptionId` describes. */
	readonly badge?: React.ReactNode;
	/**
	 * The column it shows and hides: the Sidebar, whose button stands at the window's start, or the
	 * File Bar, whose button stands at its end on either side of the terminal area.
	 */
	readonly column: 'file-bar' | 'sidebar';
	/** The element that describes the button beyond its name, such as what its badge counts. */
	readonly descriptionId?: string | undefined;
	readonly label: string;
	readonly onToggle: () => void;
	/** Whether the column shows. */
	readonly pressed: boolean;
	readonly shortcutId: string;
	/** Absent while a configured command holds the chord. */
	readonly shortcutLabel: string | undefined;
	/** The File Bar's side of the terminal area, which its icon mirrors as VS Code's does. */
	readonly side?: 'left' | 'right' | undefined;
	/** Touch sizes: a 36 px fill in a 44 px target. */
	readonly touch?: boolean | undefined;
}

function columnToggleIcon(
	column: WorkbenchColumnToggleProps['column'],
	pressed: boolean,
	side: WorkbenchColumnToggleProps['side'],
): ProductIconName {
	if (column === 'sidebar') return 'sidebar-left';
	if (side === 'left') return pressed ? 'sidebar-left' : 'sidebar-left-off';
	return pressed ? 'sidebar-right' : 'sidebar-right-off';
}

/** The button that shows and hides a column, as VS Code's title bar toggles its side bars. */
export const WorkbenchColumnToggle = React.memo(function WorkbenchColumnToggle({
	badge,
	column,
	descriptionId,
	label,
	onToggle,
	pressed,
	shortcutId,
	shortcutLabel,
	side,
	touch = false,
}: WorkbenchColumnToggleProps): React.ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-describedby={descriptionId}
						aria-label={label}
						aria-pressed={pressed}
						className={styles['trigger']}
						data-shortcut-action={shortcutId}
						data-slot={
							column === 'sidebar' ? 'workbench-sidebar-toggle' : 'workbench-file-bar-toggle'
						}
						data-touch={touch || undefined}
						size='icon-sm'
						variant='ghost'
						onClick={onToggle}
					/>
				}
			>
				<ProductIcon name={columnToggleIcon(column, pressed, side)} />
				{badge}
			</TooltipTrigger>
			{/* The tooltip opens toward the window's middle, so it stays in the window. */}
			<TooltipContent align={column === 'sidebar' ? 'start' : 'end'} side='bottom'>
				{label}
				{shortcutLabel === undefined ? null : <Kbd>{shortcutLabel}</Kbd>}
			</TooltipContent>
		</Tooltip>
	);
});
