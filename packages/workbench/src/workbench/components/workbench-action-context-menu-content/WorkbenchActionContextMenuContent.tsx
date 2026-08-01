import {
	ContextMenuContent,
	ContextMenuGroup,
	ContextMenuItem,
	ContextMenuLabel,
	ContextMenuSeparator,
	ContextMenuSub,
	ContextMenuSubContent,
	ContextMenuSubTrigger,
	ProductIcon,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type { WorkbenchActionMenuEntry } from '../../types/workbench-action-menu';

interface WorkbenchActionContextMenuContentProps {
	readonly entries: readonly WorkbenchActionMenuEntry[];
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
							</ContextMenuItem>
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
											<ContextMenuLabel>{group.label}</ContextMenuLabel>
											{group.commands.map((command) => (
												<ContextMenuItem
													key={command.id}
													disabled={command.disabled}
													variant={command.destructive === true ? 'destructive' : 'default'}
													onClick={command.onSelect}
												>
													<ProductIcon name={command.icon} size='menu-leading' />
													{command.label}
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
