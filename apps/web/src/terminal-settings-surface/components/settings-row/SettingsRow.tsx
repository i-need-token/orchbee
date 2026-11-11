import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import styles from './SettingsRow.module.css';

interface SettingsRowProps {
	/** The row's one control, or the action or switch of a list entry. */
	readonly children?: React.ReactNode;
	readonly description?: string | null | undefined;
	/** Colors a description that reports a state, such as an integration's health. */
	readonly descriptionTone?: 'danger' | 'neutral' | 'positive' | 'warning' | undefined;
	/** When a change takes effect, if not at once. */
	readonly effect?: 'new-terminals' | 'next-restart' | undefined;
	/** Supporting text beside the name, such as a command path. */
	readonly detail?: string | null | undefined;
	readonly label: string;
	readonly modified?: boolean | undefined;
	readonly onRestore?: (() => void) | undefined;
	/** The global Settings query; a row that does not match it renders nothing. */
	readonly query: string;
	/** Extra words that find this row, such as a shortcut chord. */
	readonly searchTerms?: readonly string[] | undefined;
	readonly settingId?: string | undefined;
	/** Messages under the description: validation, progress, or a refused change. */
	readonly status?: React.ReactNode;
}

/** One setting or list entry: name, an always-visible description, and one trailing control. */
export function SettingsRow({
	children,
	description,
	descriptionTone,
	detail,
	effect,
	label,
	modified = false,
	onRestore,
	query,
	searchTerms = [],
	settingId,
	status,
}: SettingsRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	if (!matchesSettingsQuery(query, [label, description ?? '', ...searchTerms], intl.locale)) {
		return null;
	}
	const restoreLabel = intl.formatMessage(terminalSettingsMessages.pageRestoreDefault);
	return (
		<div
			className={styles['root']}
			data-modified={modified || undefined}
			data-settings-row
			data-setting-id={settingId}
			data-two-line={description === null || description === undefined ? undefined : true}
			id={settingId}
			tabIndex={settingId === undefined ? undefined : -1}
		>
			{modified ? (
				<span
					aria-hidden='true'
					className={styles['modified']}
					title={intl.formatMessage(terminalSettingsMessages.pageChangedFromDefault)}
				/>
			) : null}
			<div className={styles['copy']}>
				<div className={styles['nameLine']}>
					<span className={styles['name']}>{label}</span>
					{detail === null || detail === undefined ? null : (
						<span className={styles['detail']}>{detail}</span>
					)}
					{effect === undefined ? null : (
						<span className={styles['effect']}>
							{intl.formatMessage(
								effect === 'new-terminals'
									? terminalSettingsMessages.pageEffectNewTerminals
									: terminalSettingsMessages.pageEffectNextRestart,
							)}
						</span>
					)}
				</div>
				{description === null || description === undefined ? null : (
					<div className={styles['description']} data-tone={descriptionTone}>
						{description}
					</div>
				)}
				{status}
			</div>
			<div className={styles['control']}>
				{modified && onRestore !== undefined ? (
					<button
						aria-label={`${restoreLabel}: ${label}`}
						className={styles['restore']}
						title={restoreLabel}
						type='button'
						onClick={onRestore}
					>
						<ProductIcon name='restore' />
					</button>
				) : null}
				{children}
			</div>
		</div>
	);
}
