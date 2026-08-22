import {
	DropdownMenuContent,
	DropdownMenuGroup,
	DropdownMenuItem,
	DropdownMenuLabel,
	DropdownMenuSeparator,
	DropdownMenuShortcut,
	DropdownMenuSub,
	DropdownMenuSubContent,
	DropdownMenuSubTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { WorkbenchActionMenuEntry } from '../../types/workbench-action-menu';

interface WorkbenchActionDropdownMenuContentProps {
	readonly entries: readonly WorkbenchActionMenuEntry[];
}

export function WorkbenchActionDropdownMenuContent({
	entries,
}: WorkbenchActionDropdownMenuContentProps): React.ReactNode {
	return (
		<DropdownMenuContent align='end'>
			{entries.map((entry) => {
				switch (entry.kind) {
					case 'command':
						return (
							<DropdownMenuItem
								key={entry.id}
								disabled={entry.disabled}
								variant={entry.destructive === true ? 'destructive' : 'default'}
								onClick={entry.onSelect}
							>
								<ProductIcon name={entry.icon} size='menu-leading' />
								{entry.label}
								{entry.shortcut === undefined ? null : (
									<DropdownMenuShortcut>{entry.shortcut}</DropdownMenuShortcut>
								)}
							</DropdownMenuItem>
						);
					case 'separator':
						return <DropdownMenuSeparator key={entry.id} />;
					case 'submenu':
						return (
							<DropdownMenuSub key={entry.id}>
								<DropdownMenuSubTrigger>
									<ProductIcon name={entry.icon} size='menu-leading' />
									{entry.label}
								</DropdownMenuSubTrigger>
								<DropdownMenuSubContent>
									{entry.groups.map((group) => (
										<DropdownMenuGroup key={group.id}>
											{group.label === undefined ? null : (
												<DropdownMenuLabel>{group.label}</DropdownMenuLabel>
											)}
											{group.commands.map((command) => (
												<DropdownMenuItem
													key={command.id}
													disabled={command.disabled}
													variant={command.destructive === true ? 'destructive' : 'default'}
													onClick={command.onSelect}
												>
													<ProductIcon name={command.icon} size='menu-leading' />
													{command.label}
													{command.shortcut === undefined ? null : (
														<DropdownMenuShortcut>{command.shortcut}</DropdownMenuShortcut>
													)}
												</DropdownMenuItem>
											))}
										</DropdownMenuGroup>
									))}
								</DropdownMenuSubContent>
							</DropdownMenuSub>
						);
				}
			})}
		</DropdownMenuContent>
	);
}
