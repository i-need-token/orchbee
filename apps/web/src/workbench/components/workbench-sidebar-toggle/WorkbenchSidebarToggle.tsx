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
	readonly shortcutLabel: string;
}

export function WorkbenchSidebarToggle({
	isSidebarVisible,
	label,
	onToggle,
	shortcutId,
	shortcutLabel,
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
				<Kbd>{shortcutLabel}</Kbd>
			</TooltipContent>
		</Tooltip>
	);
}
