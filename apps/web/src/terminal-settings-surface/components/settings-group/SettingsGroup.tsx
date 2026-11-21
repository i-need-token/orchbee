import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { ProductIcon, Tooltip, TooltipContent, TooltipTrigger } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './SettingsGroup.module.css';

interface SettingsGroupProps {
	/** A quiet action beside the heading, such as reloading a list. */
	readonly action?: React.ReactNode;
	/** Entries of the group's single card. */
	readonly children: React.ReactNode;
	readonly description?: string | null | undefined;
	/** When the group's changes take effect, if not at once. */
	readonly effect?: 'new-terminals' | undefined;
	/** Omitted for an unnamed card. */
	readonly label?: string | undefined;
	/** A list that is one setting shows its changed state and restore on the heading. */
	readonly modified?: boolean | undefined;
	/** A closing line under the card. */
	readonly note?: React.ReactNode;
	readonly onRestore?: (() => void) | undefined;
	readonly tag?: string | null | undefined;
}

/**
 * A named or unnamed group: a small muted heading, an optional one-line note, one card, and an
 * optional closing line. A group whose card holds no visible entry, as during a search, is hidden
 * by its stylesheet.
 */
export function SettingsGroup({
	action,
	children,
	description,
	effect,
	label,
	modified = false,
	note,
	onRestore,
	tag,
}: SettingsGroupProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const headingId = React.useId();
	const restoreLabel = intl.formatMessage(terminalSettingsMessages.pageRestoreDefault);
	return (
		<section
			aria-labelledby={label === undefined ? undefined : headingId}
			className={styles['root']}
			data-named={label === undefined ? undefined : true}
			data-slot='settings-group'
		>
			{label === undefined ? null : (
				<div className={styles['heading']}>
					<h2 className={styles['label']} id={headingId}>
						{label}
						{effect === undefined
							? null
							: ` · ${intl.formatMessage(terminalSettingsMessages.pageEffectNewTerminals)}`}
					</h2>
					{modified ? (
						<Tooltip>
							<TooltipTrigger render={<span aria-hidden='true' className={styles['modified']} />} />
							<TooltipContent side='top'>
								{intl.formatMessage(terminalSettingsMessages.pageChangedFromDefault)}
							</TooltipContent>
						</Tooltip>
					) : null}
					{tag === null || tag === undefined ? null : <span className={styles['tag']}>{tag}</span>}
					<span className={styles['actions']}>
						{modified && onRestore !== undefined ? (
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
						) : null}
						{action}
					</span>
				</div>
			)}
			{description === null || description === undefined ? null : (
				<p className={styles['description']}>{description}</p>
			)}
			<div className={styles['card']}>{children}</div>
			{note === undefined || note === null ? null : <p className={styles['note']}>{note}</p>}
		</section>
	);
}
