import {
	Button,
	Kbd,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchSidebarToggle.module.css';

interface WorkbenchSidebarToggleProps {
	readonly isSidebarVisible: boolean;
	readonly label: string;
	readonly onToggle: () => void;
	readonly shortcutId: string;
	/** Absent while a configured command holds the chord. */
	readonly shortcutLabel: string | undefined;
	/** Touch sizes: a 36 px fill in a 44 px target. */
	readonly touch?: boolean | undefined;
}

export function WorkbenchSidebarToggle({
	isSidebarVisible,
	label,
	onToggle,
	shortcutId,
	shortcutLabel,
	touch = false,
}: WorkbenchSidebarToggleProps): React.ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<Button
						aria-label={label}
						aria-pressed={isSidebarVisible}
						className={styles['trigger']}
						data-shortcut-action={shortcutId}
						data-slot='workbench-sidebar-toggle'
						data-touch={touch || undefined}
						size='icon-sm'
						variant='ghost'
						onClick={onToggle}
					/>
				}
			>
				<ProductIcon name='sidebar-left' />
			</TooltipTrigger>
			<TooltipContent align='start' side='bottom'>
				{label}
				{shortcutLabel === undefined ? null : <Kbd>{shortcutLabel}</Kbd>}
			</TooltipContent>
		</Tooltip>
	);
}
