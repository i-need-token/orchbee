import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Input } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './SettingsListFilter.module.css';

interface SettingsListFilterProps {
	/** Extra controls after the input, such as recording a shortcut to filter by. */
	readonly children?: React.ReactNode;
	readonly onChange: (value: string) => void;
	readonly placeholder: string;
	readonly shown: number;
	readonly total: number;
	readonly value: string;
}

/** The first line of a long list's card: a filter box and how many entries it shows. */
export function SettingsListFilter({
	children,
	onChange,
	placeholder,
	shown,
	total,
	value,
}: SettingsListFilterProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-settings-row data-slot='settings-list-filter'>
			<Input
				aria-label={placeholder}
				className={styles['input']}
				placeholder={placeholder}
				type='search'
				value={value}
				onChange={(event) => onChange(event.currentTarget.value)}
				onKeyDown={(event) => {
					if (event.key === 'Escape' && value !== '') {
						event.preventDefault();
						onChange('');
					}
				}}
			/>
			{children}
			<span aria-live='polite' className={styles['count']}>
				{intl.formatMessage(terminalSettingsMessages.pageFilterCount, { shown, total })}
			</span>
		</div>
	);
}
