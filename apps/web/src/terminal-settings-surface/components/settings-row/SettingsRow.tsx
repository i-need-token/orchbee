import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalSettingsMachineColor } from '../../types/terminal-settings-surface';
import { matchesSettingsQuery } from '../../utils/matchesSettingsQuery';
import styles from './SettingsRow.module.css';

interface SettingsRowProps {
	/** The row's one control, or the action or switch of a list entry. */
	readonly children?: React.ReactNode;
	readonly description?: string | null | undefined;
	/** Colors a description that reports a state, such as an update check's result. */
	readonly descriptionTone?: 'danger' | 'neutral' | 'positive' | 'warning' | undefined;
	/** When a change takes effect, if not at once. */
	readonly effect?: 'new-terminals' | 'next-restart' | undefined;
	/** Supporting text beside the name, such as a command path. */
	readonly detail?: string | null | undefined;
	/** A mark before the name, such as an Agent's product mark; the description lines up after it. */
	readonly icon?: React.ReactNode;
	readonly label: string;
	/** A saved machine's color, which its name is written in. */
	readonly machineColor?: TerminalSettingsMachineColor | undefined;
	readonly modified?: boolean | undefined;
	readonly onRestore?: (() => void) | undefined;
	/**
	 * A picture of the value under the row's name and control, across the row, as the reference's
	 * code sample under its font row. Put it on the row's last setting that changes its height, so
	 * a growing picture never moves a control under the pointer.
	 */
	readonly preview?: React.ReactNode;
	/** The global Settings query; a row that does not match it renders nothing. */
	readonly query: string;
	/** Extra words that find this row, such as a shortcut chord. */
	readonly searchTerms?: readonly string[] | undefined;
	readonly settingId?: string | undefined;
	/**
	 * The row's notices: a `SettingFailure` shows on the dialog's notice line, so an action never
	 * changes the row's height. Only a note that follows the chosen value may stand inline.
	 */
	readonly status?: React.ReactNode;
}

/**
 * One setting or list entry: name, an always-visible description, and one trailing control. A
 * restorable row keeps Restore's place whether or not its value is changed, so the control never
 * shifts.
 */
export function SettingsRow({
	children,
	description,
	descriptionTone,
	detail,
	effect,
	icon,
	label,
	machineColor,
	modified = false,
	onRestore,
	preview,
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
	let restore: React.ReactNode = null;
	if (onRestore !== undefined) {
		restore = modified ? (
			<Tooltip>
				<TooltipTrigger
					render={
						<button
							aria-label={`${restoreLabel}: ${label}`}
							className={styles['restore']}
							type='button'
							onClick={onRestore}
						>
							<ProductIcon name='restore' />
						</button>
					}
				/>
				<TooltipContent side='top'>{restoreLabel}</TooltipContent>
			</Tooltip>
		) : (
			<span aria-hidden='true' className={styles['restoreSlot']} />
		);
	}
	return (
		<div
			className={styles['root']}
			data-modified={modified || undefined}
			data-preview={preview === undefined ? undefined : true}
			data-settings-row
			data-setting-id={settingId}
			data-icon={icon === undefined ? undefined : true}
			data-two-line={description === null || description === undefined ? undefined : true}
			id={settingId}
			tabIndex={settingId === undefined ? undefined : -1}
		>
			<div className={styles['copy']}>
				<div className={styles['nameLine']}>
					{icon === undefined ? null : <span className={styles['icon']}>{icon}</span>}
					<span className={styles['name']} data-machine-color={machineColor}>
						{label}
					</span>
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
					{modified ? (
						<Tooltip>
							<TooltipTrigger render={<span aria-hidden='true' className={styles['modified']} />} />
							<TooltipContent side='top'>
								{intl.formatMessage(terminalSettingsMessages.pageChangedFromDefault)}
							</TooltipContent>
						</Tooltip>
					) : null}
				</div>
				{description === null || description === undefined ? null : (
					<div className={styles['description']} data-tone={descriptionTone}>
						{description}
					</div>
				)}
				{status}
			</div>
			<div className={styles['control']}>
				{restore}
				{children}
			</div>
			{preview === undefined ? null : <div className={styles['preview']}>{preview}</div>}
		</div>
	);
}
