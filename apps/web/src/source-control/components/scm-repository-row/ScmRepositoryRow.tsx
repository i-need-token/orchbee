import { sourceControlMessages, useOrchbeeIntl, type IntlShape } from '@orchbee/i18n';
import {
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { ScmCheckoutItem, ScmSyncItem, ScmSyncTooltip } from '../../model/scm-repository-row';
import styles from './ScmRepositoryRow.module.css';

interface ScmRepositoryRowProps {
	readonly checkout: ScmCheckoutItem;
	/** Disambiguates repositories that share a name. */
	readonly description: string | null;
	readonly name: string;
	/** Runs the sync item's action: Sync Changes, or Publish Branch for a branch without upstream. */
	readonly onSync: ((action: 'publish' | 'sync') => void) | null;
	readonly path: string;
	readonly sync: ScmSyncItem | null;
}

function checkoutTooltip(intl: IntlShape, checkout: ScmCheckoutItem): string {
	const head = checkout.rebasing
		? intl.formatMessage(sourceControlMessages.checkoutRebasing, { head: checkout.label })
		: checkout.label;
	switch (checkout.activity) {
		case 'commit':
			return intl.formatMessage(sourceControlMessages.checkoutCommitting, { head });
		case 'sync':
			return intl.formatMessage(sourceControlMessages.checkoutSyncing, { head });
		case 'none':
			return head;
	}
}

function syncTooltip(intl: IntlShape, tooltip: ScmSyncTooltip): string | null {
	switch (tooltip.kind) {
		case 'committing':
			return intl.formatMessage(sourceControlMessages.syncCommitting);
		case 'none':
			return null;
		case 'publish':
			return intl.formatMessage(sourceControlMessages.syncPublish);
		case 'pull':
			return intl.formatMessage(sourceControlMessages.syncPull, {
				count: tooltip.behind,
				upstream: tooltip.upstream,
			});
		case 'pull-and-push':
			return intl.formatMessage(sourceControlMessages.syncPullAndPush, {
				ahead: tooltip.ahead,
				behind: tooltip.behind,
				upstream: tooltip.upstream,
			});
		case 'push':
			return intl.formatMessage(sourceControlMessages.syncPush, {
				count: tooltip.ahead,
				upstream: tooltip.upstream,
			});
		case 'synchronize':
			return intl.formatMessage(sourceControlMessages.syncSynchronize);
		case 'syncing':
			return intl.formatMessage(sourceControlMessages.syncSyncing);
	}
}

function syncLabel(intl: IntlShape, sync: ScmSyncItem): string | null {
	const { label } = sync;
	if (label === null) return null;
	if (label.ahead === null) {
		return intl.formatMessage(sourceControlMessages.syncIncoming, { behind: label.behind });
	}
	return intl.formatMessage(sourceControlMessages.syncBoth, {
		ahead: label.ahead,
		behind: label.behind,
	});
}

interface ScmRepositoryItemProps {
	readonly icon: ProductIconName | null;
	readonly label: string | null;
	readonly onPress: (() => void) | null;
	readonly spinning: boolean;
	readonly tooltip: string | null;
}

/** One of the row's status items, VS Code's status bar commands drawn as row Actions. */
function ScmRepositoryItem({
	icon,
	label,
	onPress,
	spinning,
	tooltip,
}: ScmRepositoryItemProps): ReactNode {
	const content = (
		<>
			{icon === null ? null : (
				<span className={styles['icon']} data-spinning={spinning ? '' : undefined}>
					<ProductIcon name={icon} size='section-action' />
				</span>
			)}
			{label === null ? null : <span className={styles['label']}>{label}</span>}
		</>
	);
	const item =
		onPress === null ? (
			<span aria-label={tooltip ?? label ?? undefined} className={styles['item']} role='img'>
				{content}
			</span>
		) : (
			<button
				aria-label={tooltip ?? label ?? undefined}
				className={styles['item']}
				type='button'
				onClick={onPress}
			>
				{content}
			</button>
		);
	if (tooltip === null) return item;
	return (
		<Tooltip>
			<TooltipTrigger render={item} />
			<TooltipContent side='bottom'>{tooltip}</TooltipContent>
		</Tooltip>
	);
}

/**
 * VS Code's repository row: the repository's name, described when another shares it, then its
 * checkout and sync items, which show the branch with its marks and the commits to pull and push.
 */
export function ScmRepositoryRow({
	checkout,
	description,
	name,
	onSync,
	path,
	sync,
}: ScmRepositoryRowProps): ReactNode {
	const intl = useOrchbeeIntl();
	const syncAction = sync?.action ?? null;
	return (
		<span
			className={styles['root']}
			title={intl.formatMessage(sourceControlMessages.repositoryTitle, { path })}
		>
			<span className={styles['repo']}>
				<ProductIcon name='repo' size='section-action' />
			</span>
			<span className={styles['name']}>{name}</span>
			{description === null ? null : <span className={styles['description']}>{description}</span>}
			<span className={styles['items']}>
				<ScmRepositoryItem
					icon={checkout.icon}
					label={checkout.label}
					spinning={false}
					tooltip={checkoutTooltip(intl, checkout)}
					onPress={null}
				/>
				{sync === null ? null : (
					<ScmRepositoryItem
						icon={sync.icon === 'syncing' ? 'sync' : sync.icon}
						label={syncLabel(intl, sync)}
						spinning={sync.icon === 'syncing'}
						tooltip={syncTooltip(intl, sync.tooltip)}
						onPress={syncAction === null || onSync === null ? null : () => onSync(syncAction)}
					/>
				)}
			</span>
		</span>
	);
}
