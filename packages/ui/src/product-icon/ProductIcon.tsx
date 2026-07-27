import * as React from 'react';

import { cn } from '#lib';

import styles from './ProductIcon.module.css';

export type ProductIconName =
	| 'action'
	| 'add'
	| 'agent'
	| 'arrow-left'
	| 'close'
	| 'ellipsis'
	| 'layout'
	| 'more-vertical'
	| 'panels'
	| 'pin'
	| 'search'
	| 'settings'
	| 'sidebar-left'
	| 'tabs'
	| 'terminal';

interface ProductIconProps {
	readonly name: ProductIconName;
}

export function ProductIcon({ name }: ProductIconProps): React.ReactNode {
	return (
		<span
			aria-hidden='true'
			className={cn(
				styles['root'],
				name === 'action' && styles['action'],
				name === 'add' && styles['add'],
				name === 'agent' && styles['agent'],
				name === 'arrow-left' && styles['arrowLeft'],
				name === 'close' && styles['close'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'layout' && styles['layout'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'panels' && styles['panels'],
				name === 'pin' && styles['pin'],
				name === 'search' && styles['search'],
				name === 'settings' && styles['settings'],
				name === 'sidebar-left' && styles['sidebarLeft'],
				name === 'tabs' && styles['tabs'],
				name === 'terminal' && styles['terminal'],
			)}
			data-icon={name}
		/>
	);
}
