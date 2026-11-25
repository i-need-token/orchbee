import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchRightBarToggle.module.css';

interface WorkbenchRightBarToggleProps {
	readonly isRightBarVisible: boolean;
	readonly label: string;
	readonly onToggle: () => void;
	readonly shortcutId: string;
	/** Absent while a configured command holds the chord. */
	readonly shortcutLabel: string | undefined;
}

/** The Tab bar's end button that shows or hides the right bar with the Workspace's files. */
export const WorkbenchRightBarToggle = React.memo(function WorkbenchRightBarToggle({
	isRightBarVisible,
	label,
	onToggle,
	shortcutId,
	shortcutLabel,
}: WorkbenchRightBarToggleProps): React.ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-label={label}
						aria-pressed={isRightBarVisible}
						className={styles['trigger']}
						data-shortcut-action={shortcutId}
						data-slot='workbench-right-bar-toggle'
						size='icon-sm'
						variant='ghost'
						onClick={onToggle}
					/>
				}
			>
				<ProductIcon name='sidebar-right' />
			</TooltipTrigger>
			<TooltipContent align='end' side='bottom'>
				{label}
				{shortcutLabel === undefined ? null : <Kbd>{shortcutLabel}</Kbd>}
			</TooltipContent>
		</Tooltip>
	);
});
