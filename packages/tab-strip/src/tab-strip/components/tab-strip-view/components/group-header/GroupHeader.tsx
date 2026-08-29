import styles from './GroupHeader.module.css';
import {
	type CSSProperties,
	type MouseEvent as ReactMouseEvent,
	type PointerEvent as ReactPointerEvent,
	type ReactNode,
} from 'react';
import { type TabData, type TabGroupData } from '../../../../model/tab-model';
import { TabStripOrientation } from '../../../../types/tab-strip-types';
import {
	calculateHorizontalTabGroupHeaderLayout,
	getTabGroupTitleVisualCenteringOffset,
} from '../../../../layout/tab-group-style';
import { TabStripIcon } from '../tab-strip-icon/TabStripIcon';
import {
	GROUP_COLORS,
	getMaxContrastForeground,
	measuredGroupTitleWidth,
} from '../../../../utils/tab-strip-internals';
import { useGroupHeader } from './hooks/useGroupHeader';
import { useTabStripMessages } from '../../../../hooks/useTabStripMessages';

export function GroupHeader({
	group,
	tabs,
	activeTabId,
	orientation,
	collapsedStrip,
	onToggle,
	onPointerDown,
	onKeyDown,
	onContextMenu,
	onOpenEditor,
	onMouseEnter,
	onMouseLeave,
	tabIndex,
	showHoverCard,
	frameCondensed = false,
}: {
	group: Readonly<TabGroupData>;
	tabs: readonly Readonly<TabData>[];
	activeTabId: string | null;
	orientation: TabStripOrientation;
	collapsedStrip: boolean;
	onToggle: (source: 'mouse' | 'touch' | 'keyboard') => void;
	onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
	onKeyDown: (event: React.KeyboardEvent<HTMLDivElement>) => void;
	onContextMenu: (event: ReactMouseEvent) => void;
	onOpenEditor?: (x: number, y: number) => void;
	onMouseEnter: (event: ReactMouseEvent<HTMLDivElement>) => void;
	onMouseLeave: () => void;
	tabIndex: -1 | 0;
	showHoverCard: boolean;
	frameCondensed?: boolean;
}): ReactNode {
	const messages = useTabStripMessages();
	const { editorOpenedOnPress, headerRef, lastPointerType } = useGroupHeader();
	const applyMacEmojiCentering =
		orientation === TabStripOrientation.Horizontal &&
		getTabGroupTitleVisualCenteringOffset(group.title, {
			isMac: typeof navigator !== 'undefined' && /Mac/.test(navigator.platform),
		}) !== 0;
	const horizontalLayout = calculateHorizontalTabGroupHeaderLayout({
		title: group.title,
		measuredTitleWidth: measuredGroupTitleWidth(group.title),
		collapsed: group.collapsed,
		tabCount: tabs.length,
		...(group.attention === undefined ? {} : { attention: group.attention }),
	});
	const groupColor = GROUP_COLORS[group.color];
	const groupForeground = getMaxContrastForeground(groupColor);
	const representedActiveTabId =
		group.collapsed && activeTabId !== null && tabs.some((tab) => tab.id === activeTabId)
			? activeTabId
			: null;
	return (
		<div
			ref={headerRef}
			role='tablist'
			tabIndex={tabIndex}
			className={styles['ct-group-header']}
			data-tab-strip-group-header=''
			data-group-id={group.id}
			data-tab-strip-roving-key={`group:${group.id}`}
			data-active={representedActiveTabId !== null || undefined}
			data-active-tab-id={representedActiveTabId ?? undefined}
			data-orientation={orientation}
			data-frame-condensed={
				(orientation === TabStripOrientation.Horizontal && frameCondensed) || undefined
			}
			data-strip-collapsed={collapsedStrip || undefined}
			aria-expanded={!group.collapsed}
			aria-label={messages.formatGroupAccessibleName(group, tabs)}
			title={showHoverCard ? '' : messages.formatGroupTooltip(group, tabs)}
			style={
				{
					'--ob-comp-tab-strip-group-color': groupColor,
					'--ob-comp-tab-strip-group-foreground': groupForeground,
				} satisfies CSSProperties
			}
			onClick={(event) => {
				let source: 'keyboard' | 'mouse' | 'touch' = 'mouse';
				if (event.detail === 0) source = 'keyboard';
				else if (lastPointerType.current === 'touch') source = 'touch';
				onToggle(source);
				event.currentTarget.focus({ preventScroll: true });
			}}
			onPointerDown={(event) => {
				lastPointerType.current = event.pointerType || 'mouse';
				onPointerDown(event);
			}}
			onKeyDown={(event) => {
				if (event.key === ' ' || event.key === 'Enter') {
					event.preventDefault();
					onToggle('keyboard');
					return;
				}
				onKeyDown(event);
			}}
			onContextMenu={onContextMenu}
			onMouseEnter={onMouseEnter}
			onMouseLeave={() => onMouseLeave()}
		>
			{orientation === TabStripOrientation.Horizontal ? (
				<span
					className={styles['ct-group-header__surface']}
					data-group-surface=''
					data-group-active-surface={representedActiveTabId !== null || undefined}
				>
					<span
						className={styles['ct-group-header__chip']}
						data-group-chip=''
						data-attention={horizontalLayout.showAttentionIndicator || undefined}
						data-has-title={Boolean(group.title) || undefined}
						style={{ '--ob-comp-tab-strip-inline-size': `${horizontalLayout.chipWidth}px` }}
					>
						{group.title ? (
							<span
								className={styles['ct-group-header__title']}
								data-mac-emoji-center={applyMacEmojiCentering || undefined}
							>
								{group.title}
							</span>
						) : null}
						{horizontalLayout.showAttentionIndicator ? (
							<span className={styles['ct-group-header__attention']} data-group-attention=''>
								<TabStripIcon name='group-attention' />
							</span>
						) : null}
					</span>
				</span>
			) : (
				<>
					<span className={styles['ct-group-header__title']}>{group.title}</span>
					{group.collapsed && group.attention && tabs.length > 0 ? (
						<span className={styles['ct-group-header__attention']} data-group-attention=''>
							<TabStripIcon name='group-attention' />
						</span>
					) : null}
					{!collapsedStrip && onOpenEditor ? (
						<button
							type='button'
							className={styles['ct-group-header__editor']}
							aria-label={messages.moreOptions}
							tabIndex={-1}
							onPointerDown={(event) => {
								if (event.button !== 0) return;
								event.preventDefault();
								event.stopPropagation();
								editorOpenedOnPress.current = true;
								const rect = event.currentTarget.getBoundingClientRect();
								onOpenEditor(rect.right, rect.bottom);
							}}
							onClick={(event) => {
								event.preventDefault();
								event.stopPropagation();
								if (editorOpenedOnPress.current) {
									editorOpenedOnPress.current = false;
									return;
								}
								const rect = event.currentTarget.getBoundingClientRect();
								onOpenEditor(rect.right, rect.bottom);
							}}
						>
							<TabStripIcon name='more-vertical' />
						</button>
					) : null}
					<TabStripIcon name='chevron' variantClass='ct-group-header__chevron' />
				</>
			)}
		</div>
	);
}
