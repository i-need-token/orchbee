import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	getVisibleTabTitle,
	TabIcon,
	type TabModel,
	type TabStatusAdornment,
	type TabStripMessages,
} from '@orchbee/tab-strip';
import { type ReactNode, useSyncExternalStore } from 'react';

import { WorkbenchTabsButton } from '../workbench-tabs-button/WorkbenchTabsButton';
import styles from './WorkbenchMobileBar.module.css';

interface WorkbenchMobileBarProps {
	/** A list covers the window, so the bar leaves the accessibility tree and the tab order. */
	readonly covered: boolean;
	readonly identityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	readonly onOpenTabList: () => void;
	readonly statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	readonly tabListOpen: boolean;
	readonly tabModel: TabModel;
	/** The strip's titles for a Tab without one and for a loading Tab. */
	readonly tabStripMessages: Pick<TabStripMessages, 'loadingTabTitle' | 'newTabTitle'>;
	readonly touch: boolean;
}

/**
 * The mobile layout's bar in place of the Tab strip: the sidebar button's place, the current Tab
 * centered on the whole bar, and `Tabs 8`. A tap anywhere between the two buttons opens the Tab
 * list. The sidebar button itself floats above the start, where it stands in every layout. Each
 * side holds both sides' contents, one of them hidden, so the two take the same width and the
 * current Tab stays centered whatever stands at either edge, a desktop window's controls included.
 */
export function WorkbenchMobileBar({
	covered,
	identityIcons,
	onOpenTabList,
	statusAdornments,
	tabListOpen,
	tabModel,
	tabStripMessages,
	touch,
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
	const title = current === null ? null : getVisibleTabTitle(current, tabStripMessages);
	// The start's copy is the place of the sidebar button, which floats above it.
	const leading = (place: boolean) => (
		<span
			aria-hidden='true'
			className={styles['leading']}
			data-sidebar-toggle-place={place ? '' : undefined}
			data-slot='workbench-mobile-bar-leading'
		/>
	);
	const end = (shown: boolean) => (
		<span aria-hidden={!shown || undefined} className={styles['endGroup']}>
			<WorkbenchTabsButton
				count={count}
				label={countLabel}
				onToggle={shown ? onOpenTabList : undefined}
				open={shown && tabListOpen}
				touch={touch}
				word={word}
			/>
			{/* A desktop window's own controls at the top end, where they draw over the page. */}
			<span className={styles['trailing']} data-slot='workbench-mobile-bar-trailing' />
		</span>
	);
	return (
		<div
			className={styles['root']}
			data-slot='workbench-mobile-bar'
			data-touch={touch || undefined}
			inert={covered}
			onClick={onOpenTabList}
		>
			<span className={styles['start']} data-slot='workbench-mobile-bar-start'>
				{leading(true)}
				{end(false)}
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
						<TabIcon
							identityIcon={identityIcons?.get(current.id)}
							statusAdornment={statusAdornments?.get(current.id)}
							tab={current}
						/>
						<span className={styles['name']}>{title}</span>
						{statusAdornments?.get(current.id)?.unreadLabel === undefined ? null : (
							<span className={styles['dot']} />
						)}
					</span>
				</button>
			)}
			<span className={styles['end']}>
				{leading(false)}
				{end(true)}
			</span>
		</div>
	);
}
