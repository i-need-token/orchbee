import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { splitPath } from '../../../../change-groups';
import type { FloatTab } from '../../../../float-tabs';

import styles from './FloatTabStrip.module.css';

interface FloatTabStripProps {
	readonly tabs: readonly FloatTab[];
	readonly activeId: string;
	readonly onActivate: (id: string) => void;
	readonly onKeep: (id: string) => void;
	readonly onClose: (id: string) => void;
}

function pathOf(tab: FloatTab): string {
	return tab.target.kind === 'file' ? tab.target.path : tab.target.item.path;
}

/**
 * The float window's own tabs. A preview tab's name is italic; double-clicking keeps it. Arrow
 * keys move focus across the tabs without switching, as in the Tab strip above, and Enter or Space
 * switches; a middle click or Delete closes the focused tab.
 */
export function FloatTabStrip({
	tabs,
	activeId,
	onActivate,
	onKeep,
	onClose,
}: FloatTabStripProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [focusedId, setFocusedId] = React.useState(activeId);
	const tabReferences = React.useRef(new Map<string, HTMLDivElement>());
	const roving = tabs.some((tab) => tab.id === focusedId) ? focusedId : activeId;
	React.useEffect(() => {
		setFocusedId(activeId);
		tabReferences.current.get(activeId)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
	}, [activeId]);
	const moveFocus = (offset: number | 'first' | 'last'): void => {
		const index = tabs.findIndex((tab) => tab.id === roving);
		let next: number;
		if (offset === 'first') next = 0;
		else if (offset === 'last') next = tabs.length - 1;
		else next = (index + offset + tabs.length) % tabs.length;
		const target = tabs[next];
		if (target === undefined) return;
		setFocusedId(target.id);
		tabReferences.current.get(target.id)?.focus();
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>, tab: FloatTab): void => {
		let handled = true;
		switch (event.key) {
			case 'ArrowRight':
				moveFocus(1);
				break;
			case 'ArrowLeft':
				moveFocus(-1);
				break;
			case 'Home':
				moveFocus('first');
				break;
			case 'End':
				moveFocus('last');
				break;
			case 'Enter':
			case ' ':
				onActivate(tab.id);
				break;
			case 'Delete':
				onClose(tab.id);
				break;
			default:
				handled = false;
		}
		if (handled) event.preventDefault();
	};
	return (
		<div
			aria-label={intl.formatMessage(workbenchMessages.rightBarFloatTabs)}
			className={styles['root']}
			role='tablist'
		>
			{tabs.map((tab) => {
				const path = pathOf(tab);
				const { name } = splitPath(path);
				const selected = tab.id === activeId;
				const suffix =
					tab.target.kind === 'diff'
						? intl.formatMessage(
								tab.target.item.after === 'index'
									? workbenchMessages.rightBarDiffSuffixStaged
									: workbenchMessages.rightBarDiffSuffixWorktree,
							)
						: null;
				return (
					<div
						aria-selected={selected}
						className={styles['tab']}
						data-preview={tab.preview || undefined}
						key={tab.id}
						onAuxClick={(event) => {
							if (event.button === 1) {
								event.preventDefault();
								onClose(tab.id);
							}
						}}
						onClick={() => onActivate(tab.id)}
						onDoubleClick={() => onKeep(tab.id)}
						onFocus={() => setFocusedId(tab.id)}
						onKeyDown={(event) => onKeyDown(event, tab)}
						ref={(element) => {
							if (element === null) tabReferences.current.delete(tab.id);
							else tabReferences.current.set(tab.id, element);
						}}
						role='tab'
						tabIndex={tab.id === roving ? 0 : -1}
						title={
							tab.preview
								? intl.formatMessage(workbenchMessages.rightBarFloatPreviewTitle, { path })
								: path
						}
					>
						<span className={styles['icon']}>
							<ProductIcon name={tab.target.kind === 'diff' ? 'diff' : 'file'} />
						</span>
						<span className={styles['name']}>{name}</span>
						{suffix === null ? null : <span className={styles['suffix']}>{suffix}</span>}
						<button
							aria-label={intl.formatMessage(workbenchMessages.rightBarFloatCloseTab, { name })}
							className={styles['close']}
							onClick={(event) => {
								event.stopPropagation();
								onClose(tab.id);
							}}
							tabIndex={-1}
							type='button'
						>
							<ProductIcon name='close' />
						</button>
					</div>
				);
			})}
		</div>
	);
}
