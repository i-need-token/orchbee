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
	WorkbenchActionMenuEntry,
} from '../../types/workbench-action-menu';

interface WorkbenchActionContextMenuContentProps {
	/** A choice shows only here: no ellipsis menu holds one. */
	readonly entries: readonly (WorkbenchActionMenuChoice | WorkbenchActionMenuEntry)[];
	readonly finalFocus?: React.ComponentProps<typeof ContextMenuContent>['finalFocus'];
}

export function WorkbenchActionContextMenuContent({
	entries,
	finalFocus,
}: WorkbenchActionContextMenuContentProps): React.ReactNode {
	return (
		<ContextMenuContent finalFocus={finalFocus}>
			{entries.map((entry) => {
				switch (entry.kind) {
					case 'command':
						return (
							<ContextMenuItem
								key={entry.id}
								disabled={entry.disabled}
								variant={entry.destructive === true ? 'destructive' : 'default'}
								onClick={entry.onSelect}
							>
								<ProductIcon name={entry.icon} size='menu-leading' />
								{entry.label}
								{entry.shortcut === undefined ? null : (
									<ContextMenuShortcut>{entry.shortcut}</ContextMenuShortcut>
								)}
							</ContextMenuItem>
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
											{group.commands.map((command) => (
												<ContextMenuItem
													key={command.id}
													disabled={command.disabled}
													variant={command.destructive === true ? 'destructive' : 'default'}
													onClick={command.onSelect}
												>
													<ProductIcon name={command.icon} size='menu-leading' />
													{command.label}
													{command.shortcut === undefined ? null : (
														<ContextMenuShortcut>{command.shortcut}</ContextMenuShortcut>
													)}
												</ContextMenuItem>
											))}
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
