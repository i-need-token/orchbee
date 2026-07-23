import type { ReactNode } from 'react';
import styles from './TabStripIconSvg.module.css';
import type { TabStripIconVariantClass } from '../../types/tab-strip-icon-variant-class';
import { joinClassNames } from '../../../../../../utils/tab-strip-internals';

interface TabStripIconSvgProps {
	children: ReactNode;
	iconName: string;
	variantClass?: TabStripIconVariantClass | undefined;
	spinner?: boolean | undefined;
	viewBox?: string | undefined;
}

export function TabStripIconSvg({
	children,
	iconName,
	variantClass,
	spinner = false,
	viewBox = '0 0 24 24',
}: TabStripIconSvgProps): ReactNode {
	return (
		<svg
			data-tab-strip-icon={iconName}
			className={joinClassNames(
				spinner ? styles['ct-spinner'] : undefined,
				variantClass === undefined ? undefined : styles[variantClass],
			)}
			viewBox={viewBox}
			width='16'
			height='16'
			fill='none'
			stroke='currentColor'
			strokeWidth='2'
			strokeLinecap='round'
			strokeLinejoin='round'
			aria-hidden='true'
		>
			{children}
		</svg>
	);
}
