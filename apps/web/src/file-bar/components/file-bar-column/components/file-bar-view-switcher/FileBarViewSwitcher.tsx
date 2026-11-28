import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	fileBarMessages,
	sourceControlMessages,
	useOrchbeeIntl,
	type MessageDescriptor,
} from '@orchbee/i18n';
import {
	Button,
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import type { KeyboardEvent, ReactNode } from 'react';

import { FILE_BAR_VIEW_IDS, type FileBarViewId } from '../../../../file-bar-layout';
import { FileBarCountBadge } from '../../../file-bar-count-badge/FileBarCountBadge';
import styles from './FileBarViewSwitcher.module.css';

const VIEW_ICONS: Readonly<Record<FileBarViewId, ProductIconName>> = {
	explorer: 'files',
	'source-control': 'source-control',
};

const VIEW_LABELS: Readonly<Record<FileBarViewId, MessageDescriptor>> = {
	explorer: fileBarMessages.explorer,
	'source-control': fileBarMessages.sourceControl,
};

interface FileBarViewSwitcherProps {
	readonly activeView: FileBarViewId;
	/** Source Control's pending changes, which its toggle's badge counts; zero shows no badge. */
	readonly changes: number;
	readonly onSelectView: (view: FileBarViewId) => void;
	readonly tabIdOf: (view: FileBarViewId) => string;
	/** The active view's region, which the selected toggle controls. */
	readonly viewId: string;
}

/**
 * The File Bar's view toggles, as VS Code's activity bar shows them at the top of its Side Bar. ← and
 * → move focus between them, Home and End to the ends, and Enter or Space selects one; Tab leaves at
 * the selected toggle. Source Control's toggle carries its count badge, which its name and tooltip
 * also say, as VS Code titles an activity with a badge.
 */
export function FileBarViewSwitcher({
	activeView,
	changes,
	onSelectView,
	tabIdOf,
	viewId,
}: FileBarViewSwitcherProps): ReactNode {
	const intl = useOrchbeeIntl();
	const onKeyDown = useMemoizedFn((event: KeyboardEvent<HTMLDivElement>) => {
		if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
		const tabs = [...event.currentTarget.querySelectorAll<HTMLElement>('[role="tab"]')];
		const current = tabs.findIndex((tab) => tab === event.target);
		if (current < 0) return;
		let next: number;
		switch (event.key) {
			case 'ArrowLeft':
				next = (current + tabs.length - 1) % tabs.length;
				break;
			case 'ArrowRight':
				next = (current + 1) % tabs.length;
				break;
			case 'Home':
				next = 0;
				break;
			case 'End':
				next = tabs.length - 1;
				break;
			default:
				return;
		}
		event.preventDefault();
		tabs[next]?.focus();
	});
	return (
		<div
			aria-label={intl.formatMessage(fileBarMessages.viewSwitcher)}
			className={styles['root']}
			role='tablist'
			onKeyDown={onKeyDown}
		>
			{FILE_BAR_VIEW_IDS.map((view) => {
				const count = view === 'source-control' ? changes : 0;
				const name = intl.formatMessage(VIEW_LABELS[view]);
				const label =
					count === 0
						? name
						: intl.formatMessage(fileBarMessages.badgeLabelled, {
								badge: intl.formatMessage(sourceControlMessages.badge, { count }),
								label: name,
							});
				const selected = view === activeView;
				return (
					<Tooltip key={view}>
						<TooltipTrigger
							render={
								<Button
									aria-controls={selected ? viewId : undefined}
									aria-label={label}
									aria-selected={selected}
									className={styles['tab']}
									data-slot='file-bar-view-toggle'
									data-view={view}
									id={tabIdOf(view)}
									role='tab'
									size='icon-sm'
									tabIndex={selected ? 0 : -1}
									variant='ghost'
									onClick={() => onSelectView(view)}
								/>
							}
						>
							<ProductIcon name={VIEW_ICONS[view]} />
							<FileBarCountBadge count={count} />
						</TooltipTrigger>
						<TooltipContent side='bottom'>{label}</TooltipContent>
					</Tooltip>
				);
			})}
		</div>
	);
}
