import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './ScmSectionHeader.module.css';

interface ScmSectionHeaderProps {
	/** The section's title Actions, which show while the pointer or focus is in the section. */
	readonly children?: ReactNode;
	readonly collapsed: boolean;
	readonly onToggle: () => void;
	readonly title: string;
}

/**
 * VS Code's 22px pane header: a disclosure that collapses the section to this header, its title and
 * its title Actions.
 */
export function ScmSectionHeader({
	children,
	collapsed,
	onToggle,
	title,
}: ScmSectionHeaderProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-slot='scm-section-header'>
			<h2 className={styles['heading']}>
				<button
					aria-expanded={!collapsed}
					aria-label={intl.formatMessage(sourceControlMessages.section, { name: title })}
					className={styles['disclosure']}
					type='button'
					onClick={onToggle}
				>
					<ProductIcon name={collapsed ? 'chevron-right' : 'chevron-down'} size='disclosure' />
					<span className={styles['title']}>{title}</span>
				</button>
			</h2>
			{collapsed ? null : <div className={styles['tools']}>{children}</div>}
		</div>
	);
}
