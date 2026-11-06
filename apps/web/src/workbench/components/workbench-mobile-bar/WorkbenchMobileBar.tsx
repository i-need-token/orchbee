import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { TabModel, TabStatusAdornment } from '@orchbee/tab-strip';
import { ProductIcon } from '@orchbee/ui/components';
import { type ReactNode, useSyncExternalStore } from 'react';

import { WorkbenchTabsButton } from '../workbench-tabs-button/WorkbenchTabsButton';
import styles from './WorkbenchMobileBar.module.css';

interface WorkbenchMobileBarProps {
	readonly identityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	readonly onOpenTabList: () => void;
	readonly statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	readonly tabListOpen: boolean;
	readonly tabModel: TabModel;
	readonly touch: boolean;
	/** What the strip calls a Tab without a title. */
	readonly untitledTabTitle: string;
}

/**
 * The mobile layout's bar in place of the Tab strip: the sidebar button's place, the current Tab
 * centered on the whole bar, and `Tabs 8`. A tap anywhere between the two buttons opens the Tab
 * list. The sidebar button itself floats above the start, where it stands in every layout.
 */
export function WorkbenchMobileBar({
	identityIcons,
	onOpenTabList,
	statusAdornments,
	tabListOpen,
	tabModel,
	touch,
	untitledTabTitle,
}: WorkbenchMobileBarProps): ReactNode {
	const intl = useOrchbeeIntl();
	const snapshot = useSyncExternalStore(
		tabModel.subscribe,
		tabModel.getSnapshot,
		tabModel.getSnapshot,
	);
	const current = snapshot.tabs.find((tab) => tab.id === snapshot.activeTabId) ?? null;
	const count = snapshot.tabs.length;
	const word = intl.formatMessage(workbenchMessages.mobileTabs);
	const countLabel = intl.formatMessage(workbenchMessages.mobileTabsLabel, { count });
	const title = current === null ? null : current.title || untitledTabTitle;
	return (
		<div
			className={styles['root']}
			data-slot='workbench-mobile-bar'
			data-touch={touch || undefined}
			onClick={onOpenTabList}
		>
			{/* The start holds the sidebar button's place and a hidden copy of `Tabs 8`, so both sides
			    take the wider width and the current Tab stays centered. */}
			<span className={styles['start']}>
				<span
					aria-hidden='true'
					className={styles['leading']}
					data-slot='workbench-mobile-bar-leading'
				/>
				<WorkbenchTabsButton
					count={count}
					label={countLabel}
					open={false}
					touch={touch}
					word={word}
				/>
			</span>
			{current === null || title === null ? (
				<span className={styles['middle']} />
			) : (
				<button
					aria-label={intl.formatMessage(workbenchMessages.mobileCurrentTab, { name: title })}
					className={styles['middle']}
					data-slot='workbench-mobile-bar-current-tab'
					type='button'
				>
					<span className={styles['tab']}>
						<span className={styles['icon']}>
							{identityIcons?.get(current.id) ?? <ProductIcon name='terminal' />}
						</span>
						<span className={styles['name']}>{title}</span>
						{statusAdornments?.get(current.id)?.unreadLabel === undefined ? null : (
							<span className={styles['dot']} />
						)}
					</span>
				</button>
			)}
			<span className={styles['end']}>
				<WorkbenchTabsButton
					count={count}
					label={countLabel}
					onToggle={onOpenTabList}
					open={tabListOpen}
					touch={touch}
					word={word}
				/>
			</span>
		</div>
	);
}
