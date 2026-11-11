import * as React from 'react';

import { cn } from '#lib';

import styles from './ProductIcon.module.css';

export type ProductIconName =
	| 'action'
	| 'add'
	| 'agent'
	| 'appearance'
	| 'arrow-down'
	| 'arrow-left'
	| 'bell'
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
	| 'keyboard'
	| 'layout'
	| 'more-vertical'
	| 'new-folder'
	| 'panels'
	| 'pin'
	| 'question'
	| 'restore'
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
	readonly size?: 'disclosure' | 'menu-leading' | 'section-action';
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
				name === 'appearance' && styles['appearance'],
				name === 'arrow-down' && styles['arrowDown'],
				name === 'arrow-left' && styles['arrowLeft'],
				name === 'bell' && styles['bell'],
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
				name === 'keyboard' && styles['keyboard'],
				name === 'layout' && styles['layout'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'new-folder' && styles['newFolder'],
				name === 'panels' && styles['panels'],
				name === 'pin' && styles['pin'],
				name === 'question' && styles['question'],
				name === 'restore' && styles['restore'],
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
