import type {
	ActionAvailability,
	ActionExecution,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import {
	TabStripOrientation,
	type TabModel,
	type TabStatusAdornment,
	type TabStripChangeCallback,
	type TabStripCloseCallback,
	type TabStripCloseResume,
	type TabStripMessages,
	type TabStripNewTabChoice,
	type TabStripNewTabRequest,
	type TabStripSelectCallback,
	type TabStripTabCreationIntent,
} from '@orchbee/tab-strip';
import {
	Button,
	ContextMenu,
	ContextMenuContent,
	ContextMenuItem,
	ContextMenuShortcut,
	ContextMenuTrigger,
	Kbd,
	ProductIcon,
} from '@orchbee/ui/components';
import { type KeyboardEvent, type ReactNode, useState, useSyncExternalStore } from 'react';

import { WorkbenchTabBar } from '../workbench-tab-bar/WorkbenchTabBar';
import { WorkbenchTabsButton } from '../workbench-tabs-button/WorkbenchTabsButton';
import { useWorkbenchTabListChoice } from './hooks/useWorkbenchTabListChoice';
import styles from './WorkbenchTabList.module.css';

interface WorkbenchTabListProps {
	readonly actionRuntime: ActionRuntime;
	readonly closeResume?: TabStripCloseResume | undefined;
	readonly focusRequest: number;
	readonly getTabCreationAvailability: (intent: TabStripTabCreationIntent) => ActionAvailability;
	readonly identityIcons?: ReadonlyMap<string, ReactNode> | undefined;
	readonly newTabChoices?: readonly TabStripNewTabChoice[] | undefined;
	/** Absent while a configured command holds the chord. */
	readonly newTabShortcutLabel: string | undefined;
	readonly onClose: () => void;
	readonly onCloseTabs: TabStripCloseCallback;
	readonly onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
	readonly onNewTab: ((request: TabStripNewTabRequest) => ActionExecution) | undefined;
	readonly onSelectTab?: TabStripSelectCallback | undefined;
	readonly onSetTabTitle: (tabId: string) => void;
	readonly onTabsChange: TabStripChangeCallback | undefined;
	/** Closed, the list stays mounted, hidden and inert, because the Tab Actions resolve through its strip. */
	readonly open: boolean;
	readonly parentActionScope: ActionScope;
	readonly statusAdornments?: ReadonlyMap<string, Readonly<TabStatusAdornment>> | undefined;
	readonly tabModel: TabModel;
	readonly tabStripMessages: TabStripMessages;
	readonly touch: boolean;
	/** The current Workspace's name, which heads its Tabs. */
	readonly workspaceName: string | null;
}

/**
 * The mobile layout's Tab list (D21): New Tab first, as in the TUI, with the strip's New Tab menu,
 * then the Workspace's vertical Tab strip at the window's full width, with its order, pins, groups,
 * menus and drags. `Tabs 8` stays at the top end, pressed, and closes the list; choosing a row,
 * the current one included, or Esc closes it too.
 */
export function WorkbenchTabList({
	actionRuntime,
	closeResume,
	focusRequest,
	getTabCreationAvailability,
	identityIcons,
	newTabChoices,
	newTabShortcutLabel,
	onClose,
	onCloseTabs,
	onKeyDown,
	onNewTab,
	onSelectTab,
	onSetTabTitle,
	onTabsChange,
	open,
	parentActionScope,
	statusAdornments,
	tabModel,
	tabStripMessages,
	touch,
	workspaceName,
}: WorkbenchTabListProps): ReactNode {
	const intl = useOrchbeeIntl();
	const count = useSyncExternalStore(
		tabModel.subscribe,
		() => tabModel.getSnapshot().tabs.length,
		() => tabModel.getSnapshot().tabs.length,
	);
	const choice = useWorkbenchTabListChoice({ onChoose: onClose, open, selectTab: onSelectTab });
	const [newTabMenuOpen, setNewTabMenuOpen] = useState(false);
	const newTabAvailable =
		onNewTab !== undefined &&
		getTabCreationAvailability({ groupId: null, kind: 'new-tab' }).kind === 'enabled';
	const newTabMenuAvailable = newTabAvailable && (newTabChoices?.length ?? 0) > 0;
	const title = intl.formatMessage(workbenchMessages.mobileTabs);
	const onListKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		onKeyDown(event);
		// Enter on the current row chooses it as a click does; the strip selects only another row.
		if (
			event.key !== 'Enter' ||
			event.altKey ||
			event.ctrlKey ||
			event.metaKey ||
			event.shiftKey ||
			!(event.target instanceof Element)
		) {
			return;
		}
		const row = event.target.closest<HTMLElement>('[data-tab-id]');
		if (row !== null && row.dataset['tabId'] === tabModel.getSnapshot().activeTabId) onClose();
	};
	return (
		// A labelled section rather than a dialog: like the sidebar list, it keeps the Workbench's
		// keyboard regions, so its strip takes focus and Esc returns it to the terminal.
		<section
			aria-label={title}
			className={styles['root']}
			data-open={open || undefined}
			data-slot='workbench-tab-list'
			data-touch={touch || undefined}
			inert={!open}
			// A click on empty space keeps the keyboard in the list, where Esc closes it.
			tabIndex={-1}
			onKeyDown={onListKeyDown}
		>
			<div className={styles['band']} data-slot='workbench-tab-list-band'>
				<WorkbenchTabsButton
					count={count}
					label={intl.formatMessage(workbenchMessages.mobileTabsLabel, { count })}
					onToggle={onClose}
					open
					touch={touch}
					word={title}
				/>
				{/* A desktop window's own controls at the top end, where they draw over the page. */}
				<span className={styles['trailing']} data-slot='workbench-tab-list-trailing' />
			</div>
			{/* Right click or a long press offers the strip's New Tab menu: how the Tab starts. */}
			<ContextMenu
				disabled={!newTabMenuAvailable}
				open={newTabMenuOpen}
				onOpenChange={setNewTabMenuOpen}
			>
				<ContextMenuTrigger className={styles['actions']}>
					<Button
						aria-disabled={!newTabAvailable}
						className={styles['action']}
						data-slot='workbench-tab-list-new-tab'
						size='sm'
						variant='ghost'
						onClick={() => {
							// The release that ends a long press opens the menu, not a Tab.
							if (!newTabAvailable || newTabMenuOpen) return;
							onNewTab({ groupId: null, source: 'button' });
							onClose();
						}}
					>
						<span className={styles['icon']}>
							<ProductIcon name='add' />
						</span>
						<span className={styles['label']}>
							{intl.formatMessage(workbenchTabMessages.newTab)}
						</span>
						{newTabShortcutLabel === undefined ? null : (
							<Kbd className={styles['shortcut']}>{newTabShortcutLabel}</Kbd>
						)}
					</Button>
				</ContextMenuTrigger>
				<ContextMenuContent aria-label={tabStripMessages.newTabMenu}>
					{newTabChoices?.map((newTabChoice) => (
						<ContextMenuItem
							key={newTabChoice.id}
							onClick={() => {
								newTabChoice.select();
								onClose();
							}}
						>
							<ProductIcon name={newTabChoice.icon} size='menu-leading' />
							{newTabChoice.label}
							{newTabChoice.note === undefined ? null : (
								<ContextMenuShortcut>{newTabChoice.note}</ContextMenuShortcut>
							)}
						</ContextMenuItem>
					))}
				</ContextMenuContent>
			</ContextMenu>
			<div
				className={styles['list']}
				onPointerCancelCapture={choice.onPointerCancelCapture}
				onPointerDownCapture={choice.onPointerDownCapture}
				onPointerUpCapture={choice.onPointerUpCapture}
			>
				{workspaceName === null ? null : (
					<div className={styles['heading']}>
						<span className={styles['headingName']}>{workspaceName}</span>
					</div>
				)}
				<WorkbenchTabBar
					actionRuntime={actionRuntime}
					closeResume={closeResume}
					fill
					focusRequest={focusRequest}
					getTabCreationAvailability={getTabCreationAvailability}
					newTabChoices={newTabChoices}
					onCloseTabs={onCloseTabs}
					onDraggingChange={choice.onDraggingChange}
					onNewTerminal={onNewTab}
					onSelectTab={choice.onSelectTab}
					onSetTabTitle={onSetTabTitle}
					onTabsChange={onTabsChange}
					onVerticalCollapsedChange={ignoreCollapse}
					orientation={TabStripOrientation.Vertical}
					parentActionScope={parentActionScope}
					reserveSidebarToggleSpace={false}
					statusAdornments={statusAdornments}
					tabIdentityIcons={identityIcons}
					tabModel={tabModel}
					tabStripMessages={tabStripMessages}
					touchUi={touch}
					verticalCollapsed={false}
				/>
			</div>
		</section>
	);
}

/** The list's strip never collapses; it offers no control for it. */
function ignoreCollapse(): void {}
