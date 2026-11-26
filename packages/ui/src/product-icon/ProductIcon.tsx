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
	| 'book'
	| 'case-sensitive'
	| 'check'
	| 'chevron-down'
	| 'chevron-right'
	| 'clear-all'
	| 'clipboard'
	| 'close'
	| 'collapse-all'
	| 'color'
	| 'copy'
	| 'diff'
	| 'edit'
	| 'ellipsis'
	| 'exclude'
	| 'expand-all'
	| 'file'
	| 'filter'
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
	| 'preserve-case'
	| 'question'
	| 'refresh'
	| 'regex'
	| 'remove'
	| 'replace'
	| 'replace-all'
	| 'restore'
	| 'save'
	| 'screen-full'
	| 'screen-normal'
	| 'search'
	| 'search-stop'
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
	| 'whitespace'
	| 'whole-word';

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
				name === 'book' && styles['book'],
				name === 'case-sensitive' && styles['caseSensitive'],
				name === 'check' && styles['check'],
				name === 'chevron-down' && styles['chevronDown'],
				name === 'chevron-right' && styles['chevronRight'],
				name === 'clear-all' && styles['clearAll'],
				name === 'clipboard' && styles['clipboard'],
				name === 'close' && styles['close'],
				name === 'collapse-all' && styles['collapseAll'],
				name === 'color' && styles['color'],
				name === 'copy' && styles['copy'],
				name === 'diff' && styles['diff'],
				name === 'edit' && styles['edit'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'exclude' && styles['exclude'],
				name === 'expand-all' && styles['expandAll'],
				name === 'file' && styles['file'],
				name === 'filter' && styles['filter'],
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
				name === 'preserve-case' && styles['preserveCase'],
				name === 'question' && styles['question'],
				name === 'refresh' && styles['refresh'],
				name === 'regex' && styles['regex'],
				name === 'remove' && styles['remove'],
				name === 'replace' && styles['replace'],
				name === 'replace-all' && styles['replaceAll'],
				name === 'restore' && styles['restore'],
				name === 'save' && styles['save'],
				name === 'screen-full' && styles['screenFull'],
				name === 'screen-normal' && styles['screenNormal'],
				name === 'search' && styles['search'],
				name === 'search-stop' && styles['searchStop'],
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
				name === 'whole-word' && styles['wholeWord'],
			)}
			data-icon={name}
			data-icon-size={size}
		/>
	);
}
