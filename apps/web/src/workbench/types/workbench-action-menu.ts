import type { ProductIconName } from '@orchbee/ui/components';
import type * as React from 'react';

export interface WorkbenchActionMenuCommand {
	readonly destructive?: boolean;
	readonly disabled?: boolean;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly kind: 'command';
	readonly label: string;
	readonly leadingMark?: React.ReactNode;
	readonly onSelect: (event: React.MouseEvent<HTMLElement>) => void;
	readonly shortcut?: string | undefined;
}

interface WorkbenchActionMenuGroup {
	readonly commands: readonly WorkbenchActionMenuCommand[];
	readonly id: string;
	readonly label?: string | undefined;
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

/** A submenu listing a whole catalog under a search field: the rows `commands` matches. */
export interface WorkbenchActionMenuSearchSubmenu {
	readonly commands: (query: string) => readonly WorkbenchActionMenuCommand[];
	readonly emptyLabel: (query: string) => string;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly kind: 'search-submenu';
	readonly label: string;
	readonly searchLabel: string;
}

/**
 * One value chosen among a few: a submenu whose options are the values, the one in force checked.
 * Choosing an option asks for it; the check moves only when `value` does.
 */
export interface WorkbenchActionMenuChoice {
	readonly icon: ProductIconName;
	readonly id: string;
	readonly kind: 'choice';
	readonly label: string;
	readonly options: readonly {
		readonly id: string;
		readonly label: string;
		readonly onSelect: () => void;
	}[];
	readonly value: string;
}

export type WorkbenchActionMenuEntry =
	| WorkbenchActionMenuCommand
	| (WorkbenchActionMenuGroup & { readonly kind: 'group' })
	| WorkbenchActionMenuSearchSubmenu
	| WorkbenchActionMenuSeparator
	| WorkbenchActionMenuSubmenu;
