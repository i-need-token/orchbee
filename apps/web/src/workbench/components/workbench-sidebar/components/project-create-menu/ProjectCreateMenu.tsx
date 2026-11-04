import {
	Button,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchActionMenuEntry } from '../../../../types/workbench-action-menu';
import type { WorkbenchProjectNavigationMessages } from '../../types/workbench-sidebar-types';
import { WorkbenchSidebarDropdownMenu } from '../workbench-sidebar-dropdown-menu/WorkbenchSidebarDropdownMenu';
import styles from './ProjectCreateMenu.module.css';

interface ProjectCreateMenuProps {
	readonly messages: WorkbenchProjectNavigationMessages;
	readonly onConnectViaSsh?: (() => void) | undefined;
	readonly onSelectFolder: () => void;
	readonly onNewFolder?: (() => void) | undefined;
	readonly selectionPending?: boolean | undefined;
}

export function ProjectCreateMenu({
	messages,
	onConnectViaSsh,
	onSelectFolder,
	onNewFolder,
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
		...(onConnectViaSsh === undefined
			? []
			: [
					{
						icon: 'terminal' as const,
						id: 'connect-project-via-ssh',
						kind: 'command' as const,
						label: messages.connectViaSsh,
						onSelect: onConnectViaSsh,
					},
				]),
		{ id: 'project-folder-actions', kind: 'separator' },
		{
			disabled: selectionPending || onNewFolder === undefined,
			icon: 'new-folder',
			id: 'create-project-folder',
			kind: 'command',
			label: messages.newFolder,
			onSelect: onNewFolder ?? (() => undefined),
		},
	];

	return (
		<Tooltip>
			<WorkbenchSidebarDropdownMenu
				entries={entries}
				trigger={
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
				triggerContent={<ProductIcon name='add' size='section-action' />}
			/>
			<TooltipContent side='top'>{messages.projectCreate}</TooltipContent>
		</Tooltip>
	);
}
