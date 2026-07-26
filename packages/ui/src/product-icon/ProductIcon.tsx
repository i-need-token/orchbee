import * as React from 'react';

import { cn } from '#lib';

import styles from './ProductIcon.module.css';

interface ProductIconProps {
	readonly name:
		| 'add'
		| 'arrow-left'
		| 'close'
		| 'ellipsis'
		| 'more-vertical'
		| 'pin'
		| 'search'
		| 'settings'
		| 'sidebar-left'
		| 'terminal';
}

export function ProductIcon({ name }: ProductIconProps): React.ReactNode {
	return (
		<span
			aria-hidden='true'
			className={cn(
				styles['root'],
				name === 'add' && styles['add'],
				name === 'arrow-left' && styles['arrowLeft'],
				name === 'close' && styles['close'],
				name === 'ellipsis' && styles['ellipsis'],
				name === 'more-vertical' && styles['moreVertical'],
				name === 'pin' && styles['pin'],
				name === 'search' && styles['search'],
				name === 'settings' && styles['settings'],
				name === 'sidebar-left' && styles['sidebarLeft'],
				name === 'terminal' && styles['terminal'],
			)}
			data-icon={name}
		/>
	);
}
