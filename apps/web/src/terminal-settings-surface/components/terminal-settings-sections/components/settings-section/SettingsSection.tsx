import * as React from 'react';

import styles from './SettingsSection.module.css';

interface SettingsSectionProps {
	readonly children: React.ReactNode;
	readonly title: string;
	readonly visible: boolean;
}

export function SettingsSection({
	children,
	title,
	visible,
}: SettingsSectionProps): React.ReactNode {
	if (!visible) return null;
	return (
		<section className={styles['root']} data-slot='terminal-settings-section'>
			<h2 className={styles['title']}>{title}</h2>
			<div className={styles['card']}>{children}</div>
		</section>
	);
}
