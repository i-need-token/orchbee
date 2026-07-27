import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { Dialog, DialogContent, Kbd, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../../types/workbench-shortcut-platform';
import type { WorkbenchSearchEvent, WorkbenchSearchState } from './model/workbench-search-state';
import { useWorkbenchSearchSurface } from './hooks/useWorkbenchSearchSurface';
import styles from './WorkbenchSearchSurface.module.css';

interface WorkbenchSearchSurfaceProps {
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly state: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }>;
	readonly update: React.Dispatch<WorkbenchSearchEvent>;
}

export function WorkbenchSearchSurface({
	actionRuntime,
	actionScope,
	shortcutPlatform,
	state,
	update,
}: WorkbenchSearchSurfaceProps): React.ReactNode {
	const view = useWorkbenchSearchSurface({
		platform: shortcutPlatform,
		runtime: actionRuntime,
		scope: actionScope,
		state,
		update,
	});

	return (
		<Dialog onOpenChange={view.onOpenChange} open>
			<DialogContent
				aria-description={view.dialogDescription}
				aria-label={view.dialogTitle}
				className={styles['surface']}
				data-slot='workbench-search-surface'
				showCloseButton={false}
			>
				<div className={styles['inputRegion']}>
					<ProductIcon name='search' />
					<input
						aria-activedescendant={view.activeDescendant}
						aria-autocomplete='list'
						aria-controls={view.listId}
						aria-expanded='true'
						aria-label={view.dialogTitle}
						autoComplete='off'
						autoFocus
						className={styles['input']}
						data-slot='workbench-search-input'
						placeholder={view.placeholder}
						role='combobox'
						spellCheck={false}
						value={view.query}
						onChange={view.onInputChange}
						onKeyDown={view.onInputKeyDown}
					/>
				</div>
				{view.isPickingArgument ? (
					<div className={styles['pickerPrompt']} data-slot='workbench-search-picker-prompt'>
						<ProductIcon name='arrow-left' />
						<span>{view.pickerPrompt}</span>
					</div>
				) : (
					<div
						aria-label={view.filtersLabel}
						className={styles['filters']}
						data-slot='workbench-search-filters'
						role='tablist'
					>
						{view.filters.map((filter) => (
							<button
								aria-selected={filter.selected}
								className={styles['filter']}
								data-selected={filter.selected}
								key={filter.id}
								role='tab'
								tabIndex={filter.selected ? 0 : -1}
								type='button'
								onClick={filter.onSelect}
								onKeyDown={filter.onKeyDown}
								onPointerDown={filter.onPointerDown}
							>
								{filter.label}
							</button>
						))}
					</div>
				)}
				<div
					aria-label={view.resultsLabel}
					className={styles['results']}
					data-slot='workbench-search-results'
					id={view.listId}
					role='listbox'
				>
					{view.sections.length === 0 ? (
						<div className={styles['empty']} data-slot='workbench-search-empty'>
							{view.emptyMessage}
						</div>
					) : (
						view.sections.map((section) => (
							<div
								aria-labelledby={section.labelId}
								className={styles['section']}
								key={section.id}
								role='group'
							>
								<div className={styles['sectionLabel']} id={section.labelId}>
									{section.label}
								</div>
								{section.rows.map((row) => (
									<div
										aria-disabled={row.disabled}
										aria-selected={row.selected}
										className={styles['row']}
										data-disabled={row.disabled}
										data-highlighted={row.selected}
										id={row.elementId}
										key={row.id}
										ref={row.elementRef}
										role='option'
										onClick={row.onInvoke}
										onPointerDown={row.onPointerDown}
										onPointerMove={row.onPointerMove}
									>
										<span className={styles['rowIcon']}>
											<ProductIcon name={row.icon} />
										</span>
										<span className={styles['rowContent']}>
											<span className={styles['rowLabel']}>{row.label}</span>
											{(row.disabledReason ?? row.description) !== undefined && (
												<span className={styles['rowDescription']}>
													{row.disabledReason ?? row.description}
												</span>
											)}
										</span>
										<span className={styles['rowTrailing']}>
											{row.shortcut === undefined ? null : <Kbd>{row.shortcut}</Kbd>}
											{row.showsPicker && <span aria-hidden='true'>›</span>}
										</span>
									</div>
								))}
							</div>
						))
					)}
				</div>
				<div className={styles['footer']} data-slot='workbench-search-footer'>
					<span className={styles['footerHint']}>
						<span className={styles['keyGroup']}>
							<Kbd>↑</Kbd>
							<Kbd>↓</Kbd>
						</span>
						{view.footerSelect}
					</span>
					<span className={styles['footerHint']}>
						<Kbd>↵</Kbd>
						{view.footerRun}
					</span>
					<span className={styles['footerHint']}>
						<Kbd>Esc</Kbd>
						{view.footerBackOrClose}
					</span>
				</div>
				<div aria-live='polite' className={styles['liveRegion']}>
					{view.resultCountAnnouncement} {view.liveMessage}
				</div>
			</DialogContent>
		</Dialog>
	);
}
