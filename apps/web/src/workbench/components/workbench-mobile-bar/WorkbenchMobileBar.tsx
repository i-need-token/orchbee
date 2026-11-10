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
import { useCurrentTabShown } from './useCurrentTabShown';

// Only where the runtime has one, so loading the bar never depends on it.
const graphemes =
	typeof Intl.Segmenter === 'function'
		? new Intl.Segmenter(undefined, { granularity: 'grapheme' })
		: null;

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
 * The start and the current Tab share one box, so a narrow window takes their space and never the
 * end's. The current Tab's name shows whole or ends in a whole character and a whole ellipsis, and
 * otherwise leaves the icon alone; a Tab its place cannot hold hides.
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
	const { placeRef, shown: tabShown, tabRef } = useCurrentTabShown(title !== null);
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
			<span className={styles['lead']}>
				<span className={styles['start']} data-slot='workbench-mobile-bar-start'>
					{leading(true)}
					{end(false)}
				</span>
				<span className={styles['middle']} ref={placeRef}>
					{current === null || title === null ? null : (
						<span className={styles['row']}>
							<button
								aria-label={intl.formatMessage(workbenchMessages.mobileCurrentTab, { name: title })}
								className={styles['tab']}
								data-hidden={tabShown ? undefined : ''}
								data-slot='workbench-mobile-bar-current-tab'
								ref={tabRef}
								type='button'
							>
								<TabIcon
									identityIcon={identityIcons?.get(current.id)}
									statusAdornment={statusAdornments?.get(current.id)}
									tab={current}
								/>
								<span className={styles['label']}>
									<span className={styles['name']}>
										<span className={styles['nameFloor']}>{nameFloor(title)}</span>
										<span className={styles['nameWidth']}>{title}</span>
										<span className={styles['nameText']}>{title}</span>
									</span>
									{statusAdornments?.get(current.id)?.unreadLabel === undefined ? null : (
										<span className={styles['dot']} />
									)}
								</span>
							</button>
						</span>
					)}
				</span>
			</span>
			<span className={styles['end']}>
				{leading(false)}
				{end(true)}
			</span>
		</div>
	);
}

/**
 * The narrowest the current Tab's name shows: its first character and an ellipsis, each shaped on
 * its own as the truncation draws them. A name of up to three characters is its own floor instead:
 * the ellipsis, as wide as two narrow characters, could make the floor wider than the name and so
 * widen the Tab.
 */
function nameFloor(name: string): ReactNode {
	const trimmed = name.trim();
	const characters = graphemes
		? Array.from(graphemes.segment(trimmed), ({ segment }) => segment)
		: Array.from(trimmed);
	const [first] = characters;
	if (first === undefined || characters.length <= 3) return name;
	return (
		<>
			{first}
			<span className={styles['nameEllipsis']}>…</span>
		</>
	);
}
