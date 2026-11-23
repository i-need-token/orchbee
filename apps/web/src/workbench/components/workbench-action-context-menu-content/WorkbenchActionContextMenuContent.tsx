import {
	ContextMenuContent,
	ContextMenuGroup,
	ContextMenuItem,
	ContextMenuLabel,
	ContextMenuRadioGroup,
	ContextMenuRadioItem,
	ContextMenuSeparator,
	ContextMenuShortcut,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type {
	WorkbenchActionMenuChoice,
	WorkbenchActionMenuCommand,
	WorkbenchActionMenuEntry,
} from '../../types/workbench-action-menu';
import { WorkbenchActionMenuSearchList } from '../workbench-action-menu-search-list/WorkbenchActionMenuSearchList';

interface WorkbenchActionContextMenuContentProps {
	/** A choice shows only here: no ellipsis menu holds one. */
	readonly entries: readonly (WorkbenchActionMenuChoice | WorkbenchActionMenuEntry)[];
	readonly finalFocus?: React.ComponentProps<typeof ContextMenuContent>['finalFocus'];
}

export function WorkbenchActionContextMenuContent({
	entries,
	finalFocus,
}: WorkbenchActionContextMenuContentProps): React.ReactNode {
	const commandItem = (command: WorkbenchActionMenuCommand) => (
		<ContextMenuItem
			key={command.id}
			disabled={command.disabled}
			variant={command.destructive === true ? 'destructive' : 'default'}
			onClick={command.onSelect}
		>
			{command.leadingMark ?? <ProductIcon name={command.icon} size='menu-leading' />}
			{command.label}
			{command.shortcut === undefined ? null : (
				<ContextMenuShortcut>{command.shortcut}</ContextMenuShortcut>
			)}
		</ContextMenuItem>
	);
	return (
		<ContextMenuContent finalFocus={finalFocus}>
			{entries.map((entry) => {
				switch (entry.kind) {
					case 'command':
						return commandItem(entry);
					case 'group':
						return (
							<ContextMenuGroup key={entry.id}>
								{entry.label === undefined ? null : (
									<ContextMenuLabel>{entry.label}</ContextMenuLabel>
								)}
								{entry.commands.map(commandItem)}
							</ContextMenuGroup>
						);
					case 'choice':
						return (
							<ContextMenuSub key={entry.id}>
								<ContextMenuSubTrigger>
									<ProductIcon name={entry.icon} size='menu-leading' />
									{entry.label}
								</ContextMenuSubTrigger>
								<ContextMenuSubContent aria-label={entry.label}>
									<ContextMenuRadioGroup value={entry.value}>
										{entry.options.map((option) => (
											<ContextMenuRadioItem
												key={option.id}
												closeOnClick
												value={option.id}
												onClick={option.onSelect}
											>
												{option.label}
											</ContextMenuRadioItem>
										))}
									</ContextMenuRadioGroup>
								</ContextMenuSubContent>
							</ContextMenuSub>
						);
					case 'separator':
						return <ContextMenuSeparator key={entry.id} />;
					case 'search-submenu':
						return (
							<ContextMenuSub key={entry.id}>
								<ContextMenuSubTrigger>
									<ProductIcon name={entry.icon} size='menu-leading' />
									{entry.label}
								</ContextMenuSubTrigger>
								<ContextMenuSubContent>
									<WorkbenchActionMenuSearchList entry={entry} renderCommand={commandItem} />
								</ContextMenuSubContent>
							</ContextMenuSub>
						);
					case 'submenu':
						return (
							<ContextMenuSub key={entry.id}>
								<ContextMenuSubTrigger>
									<ProductIcon name={entry.icon} size='menu-leading' />
									{entry.label}
								</ContextMenuSubTrigger>
								<ContextMenuSubContent>
									{entry.groups.map((group) => (
										<ContextMenuGroup key={group.id}>
											{group.label === undefined ? null : (
												<ContextMenuLabel>{group.label}</ContextMenuLabel>
											)}
											{group.commands.map(commandItem)}
										</ContextMenuGroup>
									))}
								</ContextMenuSubContent>
							</ContextMenuSub>
						);
				}
			})}
		</ContextMenuContent>
	);
}
