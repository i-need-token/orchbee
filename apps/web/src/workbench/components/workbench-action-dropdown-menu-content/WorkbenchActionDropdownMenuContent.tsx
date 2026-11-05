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
	readonly align?: 'end' | 'start';
	readonly alignOffset?: number | undefined;
	readonly entries: readonly WorkbenchActionMenuEntry[];
	readonly finalFocus?: React.ComponentProps<typeof DropdownMenuContent>['finalFocus'];
	readonly side?: 'bottom' | 'right';
	readonly sideOffset?: number | undefined;
}

export function WorkbenchActionDropdownMenuContent({
	align = 'end',
	alignOffset,
	entries,
	finalFocus,
	side = 'bottom',
	sideOffset,
}: WorkbenchActionDropdownMenuContentProps): React.ReactNode {
	return (
		<DropdownMenuContent
			align={align}
			alignOffset={alignOffset}
			finalFocus={finalFocus}
			side={side}
			sideOffset={sideOffset}
		>
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
