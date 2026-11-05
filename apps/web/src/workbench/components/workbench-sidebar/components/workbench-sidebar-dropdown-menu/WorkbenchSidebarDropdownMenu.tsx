import { DropdownMenu, DropdownMenuTrigger } from '@orchbee/ui/components';
import type * as React from 'react';

import { WorkbenchActionDropdownMenuContent } from '../../../workbench-action-dropdown-menu-content/WorkbenchActionDropdownMenuContent';
import type { WorkbenchActionMenuEntry } from '../../../../types/workbench-action-menu';
import { useWorkbenchSidebarDropdownMenuPlacement } from './hooks/useWorkbenchSidebarDropdownMenuPlacement';

interface WorkbenchSidebarDropdownMenuProps {
	readonly entries: readonly WorkbenchActionMenuEntry[];
	readonly onOpenChange?: ((open: boolean) => void) | undefined;
	readonly trigger: React.ComponentProps<typeof DropdownMenuTrigger>['render'];
	readonly triggerContent: React.ReactNode;
}

export function WorkbenchSidebarDropdownMenu({
	entries,
	onOpenChange,
	trigger,
	triggerContent,
}: WorkbenchSidebarDropdownMenuProps): React.ReactNode {
	const placement = useWorkbenchSidebarDropdownMenuPlacement();

	return (
		<DropdownMenu
			onOpenChange={(open) => {
				placement.onOpenChange(open);
				onOpenChange?.(open);
			}}
		>
			<DropdownMenuTrigger ref={placement.triggerRef} render={trigger}>
				{triggerContent}
			</DropdownMenuTrigger>
			<WorkbenchActionDropdownMenuContent
				align='start'
				alignOffset={placement.alignOffset}
				entries={entries}
				side={placement.side}
				sideOffset={placement.sideOffset}
			/>
		</DropdownMenu>
	);
}
