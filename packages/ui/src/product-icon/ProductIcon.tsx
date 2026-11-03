import * as React from 'react';

import { cn } from '#lib';

import styles from './ProductIcon.module.css';

export type ProductIconName =
	| 'action'
	| 'add'
	| 'agent'
	| 'arrow-down'
	| 'arrow-left'
	| 'clipboard'
	| 'chevron-right'
	| 'check'
	| 'close'
	| 'color'
	| 'copy'
	| 'edit'
	| 'ellipsis'
	| 'folder'
	| 'folder-opened'
	| 'git-branch'
	| 'layout'
	| 'more-vertical'
	| 'new-folder'
	| 'new-window'
	| 'panels'
	| 'pin'
	| 'search'
	| 'screen-full'
	| 'screen-normal'
	| 'settings'
	| 'sidebar-left'
	| 'tabs'
	| 'terminal'
	| 'trash'
	| 'ungroup';

interface ProductIconProps {
	readonly name: ProductIconName;
	readonly size?: 'menu-leading' | 'section-action';
}

export function ProductIcon({ name, size }: ProductIconProps): React.ReactNode {
	return (
		<span
			aria-hidden='true'
			className={cn(
				styles['root'],
				name === 'action' && styles['action'],
				name === 'add' && styles['add'],
				name === 'agent' && styles['agent'],
				name === 'arrow-down' && styles['arrowDown'],
				name === 'arrow-left' && styles['arrowLeft'],
				name === 'clipboard' && styles['clipboard'],
				name === 'chevron-right' && styles['chevronRight'],
				name === 'check' && styles['check'],
				name === 'close' && styles['close'],
				name === 'color' && styles['color'],
				name === 'copy' && styles['copy'],
				name === 'edit' && styles['edit'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'folder' && styles['folder'],
				name === 'folder-opened' && styles['folderOpened'],
				name === 'git-branch' && styles['gitBranch'],
				name === 'layout' && styles['layout'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'new-folder' && styles['newFolder'],
				name === 'new-window' && styles['newWindow'],
				name === 'panels' && styles['panels'],
				name === 'pin' && styles['pin'],
				name === 'search' && styles['search'],
				name === 'screen-full' && styles['screenFull'],
				name === 'screen-normal' && styles['screenNormal'],
				name === 'settings' && styles['settings'],
				name === 'sidebar-left' && styles['sidebarLeft'],
				name === 'tabs' && styles['tabs'],
				name === 'terminal' && styles['terminal'],
				name === 'trash' && styles['trash'],
				name === 'ungroup' && styles['ungroup'],
			)}
			data-icon={name}
			data-icon-size={size}
		/>
	);
}
