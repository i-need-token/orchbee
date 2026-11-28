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
	| 'chevron-down'
	| 'chevron-right'
	| 'check'
	| 'clear-all'
	| 'close'
	| 'collapse-all'
	| 'color'
	| 'copy'
	| 'edit'
	| 'ellipsis'
	| 'file'
	| 'files'
	| 'folder'
	| 'folder-opened'
	| 'git-branch'
	| 'info'
	| 'keyboard'
	| 'layout'
	| 'link'
	| 'loading'
	| 'machine'
	| 'more-vertical'
	| 'new-folder'
	| 'panels'
	| 'pin'
	| 'pin-unsaved'
	| 'question'
	| 'refresh'
	| 'remove'
	| 'restore'
	| 'search'
	| 'screen-full'
	| 'screen-normal'
	| 'select-all'
	| 'settings'
	| 'sidebar-left'
	| 'sidebar-right'
	| 'sidebar-right-off'
	| 'source-control'
	| 'split-down'
	| 'tabs'
	| 'terminal'
	| 'trash'
	| 'ungroup'
	| 'unsaved'
	| 'warning';

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
				name === 'chevron-down' && styles['chevronDown'],
				name === 'chevron-right' && styles['chevronRight'],
				name === 'check' && styles['check'],
				name === 'clear-all' && styles['clearAll'],
				name === 'close' && styles['close'],
				name === 'collapse-all' && styles['collapseAll'],
				name === 'color' && styles['color'],
				name === 'copy' && styles['copy'],
				name === 'edit' && styles['edit'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'file' && styles['file'],
				name === 'files' && styles['files'],
				name === 'folder' && styles['folder'],
				name === 'folder-opened' && styles['folderOpened'],
				name === 'git-branch' && styles['gitBranch'],
				name === 'info' && styles['info'],
				name === 'keyboard' && styles['keyboard'],
				name === 'layout' && styles['layout'],
				name === 'link' && styles['link'],
				name === 'loading' && styles['loading'],
				name === 'machine' && styles['machine'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'new-folder' && styles['newFolder'],
				name === 'panels' && styles['panels'],
				name === 'pin' && styles['pin'],
				name === 'pin-unsaved' && styles['pinUnsaved'],
				name === 'question' && styles['question'],
				name === 'refresh' && styles['refresh'],
				name === 'remove' && styles['remove'],
				name === 'restore' && styles['restore'],
				name === 'search' && styles['search'],
				name === 'screen-full' && styles['screenFull'],
				name === 'screen-normal' && styles['screenNormal'],
				name === 'select-all' && styles['selectAll'],
				name === 'settings' && styles['settings'],
				name === 'sidebar-left' && styles['sidebarLeft'],
				name === 'sidebar-right' && styles['sidebarRight'],
				name === 'sidebar-right-off' && styles['sidebarRightOff'],
				name === 'source-control' && styles['sourceControl'],
				name === 'split-down' && styles['splitDown'],
				name === 'tabs' && styles['tabs'],
				name === 'terminal' && styles['terminal'],
				name === 'trash' && styles['trash'],
				name === 'ungroup' && styles['ungroup'],
				name === 'unsaved' && styles['unsaved'],
				name === 'warning' && styles['warning'],
			)}
			data-icon={name}
			data-icon-size={size}
		/>
	);
}
