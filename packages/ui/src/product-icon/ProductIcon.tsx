import * as React from 'react';

import { cn } from '#lib';

import styles from './ProductIcon.module.css';

interface ProductIconProps {
	readonly name: 'add' | 'close' | 'sidebar-left';
}

export function ProductIcon({ name }: ProductIconProps): React.ReactNode {
	return (
		<span
			aria-hidden='true'
			className={cn(
				styles['root'],
				name === 'add' && styles['add'],
				name === 'close' && styles['close'],
				name === 'sidebar-left' && styles['sidebarLeft'],
			)}
			data-icon={name}
		/>
	);
}
