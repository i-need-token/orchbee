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
	| 'arrow-up'
	| 'bell'
	| 'check'
	| 'chevron-down'
	| 'chevron-right'
	| 'clear-all'
	| 'clipboard'
	| 'close'
	| 'color'
	| 'copy'
	| 'diff'
	| 'edit'
	| 'ellipsis'
	| 'file'
	| 'fold'
	| 'folder'
	| 'folder-opened'
	| 'git-branch'
	| 'go-to-file'
	| 'info'
	| 'keyboard'
	| 'layout'
	| 'link'
	| 'machine'
	| 'more-vertical'
	| 'new-folder'
	| 'panels'
	| 'pin'
	| 'question'
	| 'refresh'
	| 'remove'
	| 'restore'
	| 'screen-full'
	| 'screen-normal'
	| 'search'
	| 'select-all'
	| 'settings'
	| 'sidebar-left'
	| 'sidebar-right'
	| 'split-down'
	| 'tabs'
	| 'terminal'
	| 'trash'
	| 'ungroup'
	| 'warning'
	| 'whitespace';

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
				name === 'arrow-up' && styles['arrowUp'],
				name === 'bell' && styles['bell'],
				name === 'check' && styles['check'],
				name === 'chevron-down' && styles['chevronDown'],
				name === 'chevron-right' && styles['chevronRight'],
				name === 'clear-all' && styles['clearAll'],
				name === 'clipboard' && styles['clipboard'],
				name === 'close' && styles['close'],
				name === 'color' && styles['color'],
				name === 'copy' && styles['copy'],
				name === 'diff' && styles['diff'],
				name === 'edit' && styles['edit'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'file' && styles['file'],
				name === 'fold' && styles['fold'],
				name === 'folder' && styles['folder'],
				name === 'folder-opened' && styles['folderOpened'],
				name === 'git-branch' && styles['gitBranch'],
				name === 'go-to-file' && styles['goToFile'],
				name === 'info' && styles['info'],
				name === 'keyboard' && styles['keyboard'],
				name === 'layout' && styles['layout'],
				name === 'link' && styles['link'],
				name === 'machine' && styles['machine'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'new-folder' && styles['newFolder'],
				name === 'panels' && styles['panels'],
				name === 'pin' && styles['pin'],
				name === 'question' && styles['question'],
				name === 'refresh' && styles['refresh'],
				name === 'remove' && styles['remove'],
				name === 'restore' && styles['restore'],
				name === 'screen-full' && styles['screenFull'],
				name === 'screen-normal' && styles['screenNormal'],
				name === 'search' && styles['search'],
				name === 'select-all' && styles['selectAll'],
				name === 'settings' && styles['settings'],
				name === 'sidebar-left' && styles['sidebarLeft'],
				name === 'sidebar-right' && styles['sidebarRight'],
				name === 'split-down' && styles['splitDown'],
				name === 'tabs' && styles['tabs'],
				name === 'terminal' && styles['terminal'],
				name === 'trash' && styles['trash'],
				name === 'ungroup' && styles['ungroup'],
				name === 'warning' && styles['warning'],
				name === 'whitespace' && styles['whitespace'],
			)}
			data-icon={name}
			data-icon-size={size}
		/>
	);
}
