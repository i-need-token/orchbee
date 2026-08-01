import type { ProductIconName } from '@orchbee/ui/components';
import type * as React from 'react';

export interface WorkbenchActionMenuCommand {
	readonly destructive?: boolean;
	readonly disabled?: boolean;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly kind: 'command';
	readonly label: string;
	readonly onSelect: (event: React.MouseEvent<HTMLElement>) => void;
}

interface WorkbenchActionMenuGroup {
	readonly commands: readonly WorkbenchActionMenuCommand[];
	readonly id: string;
	readonly label: string;
}

interface WorkbenchActionMenuSeparator {
	readonly id: string;
	readonly kind: 'separator';
}

interface WorkbenchActionMenuSubmenu {
	readonly groups: readonly WorkbenchActionMenuGroup[];
	readonly icon: ProductIconName;
	readonly id: string;
	readonly kind: 'submenu';
	readonly label: string;
}

export type WorkbenchActionMenuEntry =
	| WorkbenchActionMenuCommand
	| WorkbenchActionMenuSeparator
	| WorkbenchActionMenuSubmenu;
