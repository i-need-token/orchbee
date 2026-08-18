import {
	Button,
	DropdownMenu,
	DropdownMenuTrigger,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import { WorkbenchActionDropdownMenuContent } from '../../../workbench-action-dropdown-menu-content/WorkbenchActionDropdownMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../types/workbench-action-menu';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import styles from './ProjectCreateMenu.module.css';

interface ProjectCreateMenuProps {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly onSelectFolder: () => void;
	readonly selectionPending?: boolean | undefined;
}

export function ProjectCreateMenu({
	messages,
	onSelectFolder,
	selectionPending = false,
}: ProjectCreateMenuProps): React.ReactNode {
	const entries: readonly WorkbenchActionMenuEntry[] = [
		{
			disabled: selectionPending,
			icon: 'folder-opened',
			id: 'select-project-folder',
			kind: 'command',
			label: messages.selectFolder,
			onSelect: onSelectFolder,
		},
		{ id: 'project-folder-actions', kind: 'separator' },
		{
			disabled: true,
			icon: 'new-folder',
			id: 'create-project-folder',
			kind: 'command',
			label: messages.newFolder,
			onSelect: () => undefined,
		},
	];

	return (
		<Tooltip>
			<DropdownMenu>
				<DropdownMenuTrigger
					render={
						<TooltipTrigger
							render={
								<Button
									aria-label={messages.projectCreate}
									className={styles['trigger']}
									size='icon-xs'
									variant='ghost'
								/>
							}
						/>
					}
				>
					<ProductIcon name='add' size='section-action' />
				</DropdownMenuTrigger>
				<WorkbenchActionDropdownMenuContent entries={entries} />
			</DropdownMenu>
			<TooltipContent side='top'>{messages.projectCreate}</TooltipContent>
		</Tooltip>
	);
}
