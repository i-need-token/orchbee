import styles from './HoverCard.module.css';
import { type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import {
	ThumbnailWaitState,
	elideFilenameWithLineLengths,
	formatHoverCardDomain,
	getGroupHoverCardData,
	getHoverCardFooterRows,
	getHoverCardTitleData,
	type HoverCardThumbnailSnapshot,
} from '../../../hovercard';
import { type TabModelSnapshot } from '../../../model';
import { TabAlertIndicatorGlyph } from '../tab-alert-indicator-glyph/TabAlertIndicatorGlyph';
import { joinClassNames, type HoverState } from '../../utils/tab-strip-internals';
import { ThumbnailVisualView } from '../thumbnail-visual-view/ThumbnailVisualView';
import { useTabStripMessages } from '../../messages';

export function HoverCard({
	hover,
	snapshot,
	showPreviews,
	showMemoryUsage,
	transitioning,
	thumbnailSnapshot,
}: {
	hover: HoverState;
	snapshot: TabModelSnapshot;
	showPreviews: boolean;
	showMemoryUsage: boolean;
	transitioning: boolean;
	thumbnailSnapshot: Readonly<HoverCardThumbnailSnapshot<string>>;
}): ReactNode {
	const messages = useTabStripMessages();
	if (hover.kind === 'group') {
		const group = snapshot.groups.find((candidate) => candidate.id === hover.id);
		if (!group) return null;
		const tabs = snapshot.tabs.filter((tab) => tab.group === group.id);
		const data = getGroupHoverCardData(group, tabs);
		return createPortal(
			<div
				className={joinClassNames(styles['ct-hover-card'], styles['ct-hover-card--group'])}
				style={{
					'--ob-comp-tab-strip-inline-start': `${hover.x}px`,
					'--ob-comp-tab-strip-block-start': `${hover.y}px`,
				}}
				data-tab-strip-hover-card=''
				data-transitioning={transitioning || undefined}
			>
				<div className={styles['ct-hover-card__body']}>
					<strong>{data.title.text}</strong>
					<ul className={styles['ct-hover-card__group-tabs']}>
						{data.tabTitles.map((title, index) => (
							<li key={tabs[index]?.id ?? index} data-filename={title.isFilename || undefined}>
								{title.text}
							</li>
						))}
					</ul>
					{data.excessText ? (
						<span className={styles['ct-hover-card__group-excess']}>{data.excessText}</span>
					) : null}
				</div>
			</div>,
			document.body,
		);
	}

	const tab = snapshot.tabs.find((candidate) => candidate.id === hover.id);
	if (!tab) return null;
	const runtimePreview = thumbnailSnapshot.targetId === tab.id ? thumbnailSnapshot : null;
	const showRuntimePreview =
		runtimePreview !== null &&
		(runtimePreview.current.presentation !== 'none' ||
			runtimePreview.fadingOut !== null ||
			runtimePreview.waitState !== ThumbnailWaitState.NotWaiting);
	const showPreview =
		showPreviews &&
		tab.id !== snapshot.activeTabId &&
		(runtimePreview ? showRuntimePreview : !tab.discarded || Boolean(tab.thumbnailUrl));
	const title = tab.title || messages.newTabTitle;
	const titleData = getHoverCardTitleData(title, tab.url);
	const displayTitle = titleData.isFilename
		? elideFilenameWithLineLengths(title, {
				first: Math.min(title.length, 34),
				second: Math.min(title.length, 26),
			})
		: title;
	const domain = formatHoverCardDomain(tab.url ?? '', tab.shouldDisplayUrl !== false);
	const footerRows = getHoverCardFooterRows(tab, { showMemoryUsage });
	let previewContent: ReactNode = <span>{messages.previewUnavailable}</span>;
	if (runtimePreview) {
		previewContent = (
			<>
				<ThumbnailVisualView visual={runtimePreview.current} />
				{runtimePreview.fadingOut ? (
					<ThumbnailVisualView
						key={`${runtimePreview.revision}:${runtimePreview.fadingOut.presentation}`}
						visual={runtimePreview.fadingOut}
						fadingOut
					/>
				) : null}
			</>
		);
	} else if (tab.crashed) previewContent = <span>{messages.tabCrashed}</span>;
	else if (tab.thumbnailUrl) previewContent = <img src={tab.thumbnailUrl} alt='' />;
	return createPortal(
		<div
			className={styles['ct-hover-card']}
			style={{
				'--ob-comp-tab-strip-inline-start': `${hover.x}px`,
				'--ob-comp-tab-strip-block-start': `${hover.y}px`,
			}}
			data-tab-strip-hover-card=''
			data-transitioning={transitioning || undefined}
		>
			{showPreview ? (
				<div
					className={styles['ct-hover-card__preview']}
					data-crashed={
						runtimePreview?.current.presentation === 'crashed' ||
						(!runtimePreview && tab.crashed) ||
						undefined
					}
					data-placeholder={
						runtimePreview?.current.presentation === 'placeholder' ||
						(!runtimePreview && !tab.crashed && !tab.thumbnailUrl) ||
						undefined
					}
					data-thumbnail-wait-state={runtimePreview?.waitState}
				>
					{previewContent}
				</div>
			) : null}
			<div className={styles['ct-hover-card__body']}>
				<strong data-filename={titleData.isFilename || undefined}>{displayTitle}</strong>
				{domain ? <span data-hover-card-domain=''>{domain}</span> : null}
			</div>
			{footerRows.length > 0 ? (
				<div className={styles['ct-hover-card__footer']}>
					{footerRows.map((row) => {
						const alertClassName =
							row.kind === 'alert' ? styles['ct-hover-card__alert'] : undefined;
						const performanceClassName =
							row.kind === 'performance' ? styles['ct-hover-card__performance'] : undefined;
						return (
							<div
								key={row.kind}
								className={joinClassNames(
									styles['ct-hover-card__footer-row'],
									alertClassName,
									performanceClassName,
								)}
								data-alert-color={row.kind === 'alert' ? row.colorRole : undefined}
								data-high={(row.kind === 'performance' && row.high) || undefined}
							>
								{row.kind === 'alert' ? <TabAlertIndicatorGlyph state={row.alertState} /> : null}
								{row.kind === 'discard' || row.kind === 'performance' ? (
									<span className={styles['ct-hover-card__resource-icon']} aria-hidden='true'>
										◴
									</span>
								) : null}
								<span>{row.text}</span>
							</div>
						);
					})}
				</div>
			) : null}
		</div>,
		document.body,
	);
}
