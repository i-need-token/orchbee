import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { Dialog, DialogContent, Kbd, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { EMPTY_WORKBENCH_SEARCH_PROVIDERS } from '../../constants/search-providers';
import type { WorkbenchSearchProvider } from '../../types/workbench-search-provider';
import type { WorkbenchSearchEvent, WorkbenchSearchState } from './model/workbench-search-state';
import { useWorkbenchSearchSurface } from './hooks/useWorkbenchSearchSurface';
import styles from './WorkbenchSearchSurface.module.css';

interface WorkbenchSearchSurfaceProps {
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly providers?: readonly WorkbenchSearchProvider[] | undefined;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly state: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }>;
	readonly update: React.Dispatch<WorkbenchSearchEvent>;
}

export function WorkbenchSearchSurface({
	actionRuntime,
	actionScope,
	providers = EMPTY_WORKBENCH_SEARCH_PROVIDERS,
	shortcutLabelForAction,
	state,
	update,
}: WorkbenchSearchSurfaceProps): React.ReactNode {
	const view = useWorkbenchSearchSurface({
		providers,
		runtime: actionRuntime,
		scope: actionScope,
		shortcutLabelForAction,
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
						ref={view.inputElementRef}
						role='combobox'
						spellCheck={false}
						value={view.query}
						onChange={view.onInputChange}
						onKeyDown={view.onInputKeyDown}
					/>
				</div>
				{view.isPickingArgument ? (
					<div className={styles['pickerPrompt']} data-slot='workbench-search-picker-prompt'>
						<button
							aria-label={view.pickerBackLabel}
							className={styles['pickerBack']}
							type='button'
							onClick={view.onPickerBack}
							onPointerDown={(event) => event.preventDefault()}
						>
							<ProductIcon name='arrow-left' />
						</button>
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
				{view.providerStatuses.length === 0 ? null : (
					<div
						className={styles['providerStatuses']}
						data-slot='workbench-search-provider-statuses'
					>
						{view.providerStatuses.map((status) => (
							<div
								aria-live={status.phase === 'loading' ? 'polite' : undefined}
								className={styles['providerStatus']}
								data-phase={status.phase}
								key={status.id}
								role='status'
							>
								<span>{status.message}</span>
								{status.onRetry === undefined ? null : (
									<button
										className={styles['providerRetry']}
										type='button'
										onClick={status.onRetry}
									>
										{view.retryLabel}
									</button>
								)}
							</div>
						))}
					</div>
				)}
				<div
					aria-label={view.resultsLabel}
					className={styles['results']}
					data-slot='workbench-search-results'
					id={view.listId}
					ref={view.resultsElementRef}
					role='listbox'
					onScroll={view.onResultsScroll}
				>
					{view.showEmpty ? (
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
											{row.showsPicker && (
												<span aria-hidden='true' className={styles['rowPickerIndicator']}>
													›
												</span>
											)}
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
