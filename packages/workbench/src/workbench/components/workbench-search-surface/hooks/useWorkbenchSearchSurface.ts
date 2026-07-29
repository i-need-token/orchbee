import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { ProductIconName } from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../../../types/workbench-shortcut-platform';
import type { WorkbenchSearchFilter } from '../../../types/workbench-search';
import {
	formatWorkbenchActionMessage,
	projectWorkbenchSearchCatalog,
} from '../model/project-workbench-search-catalog';
import {
	filterWorkbenchSearchArgumentOptions,
	isWorkbenchSearchResultEnabled,
	resolveWorkbenchSearchHighlight,
	selectWorkbenchSearchSections,
} from '../model/workbench-search-ranking';
import type { WorkbenchSearchResult } from '../model/workbench-search-result';
import type { WorkbenchSearchEvent, WorkbenchSearchState } from '../model/workbench-search-state';

interface UseWorkbenchSearchSurfaceOptions {
	readonly platform: WorkbenchShortcutPlatform;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly state: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }>;
	readonly update: React.Dispatch<WorkbenchSearchEvent>;
}

interface WorkbenchSearchFilterView {
	readonly id: WorkbenchSearchFilter;
	readonly label: string;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLButtonElement>;
	readonly onPointerDown: React.PointerEventHandler<HTMLButtonElement>;
	readonly onSelect: () => void;
	readonly selected: boolean;
}

interface WorkbenchSearchRowView {
	readonly description?: string | undefined;
	readonly disabled: boolean;
	readonly disabledReason?: string | undefined;
	readonly elementId: string;
	readonly elementRef?: React.Ref<HTMLDivElement> | undefined;
	readonly icon: ProductIconName;
	readonly id: string;
	readonly label: string;
	readonly onInvoke: () => void;
	readonly onPointerDown: React.PointerEventHandler<HTMLDivElement>;
	readonly onPointerMove: () => void;
	readonly selected: boolean;
	readonly shortcut?: string | undefined;
	readonly showsPicker: boolean;
}

interface WorkbenchSearchSectionView {
	readonly id: string;
	readonly label: string;
	readonly labelId: string;
	readonly rows: readonly WorkbenchSearchRowView[];
}

interface WorkbenchSearchSurfaceView {
	readonly activeDescendant: string | undefined;
	readonly dialogDescription: string;
	readonly dialogTitle: string;
	readonly emptyMessage: string;
	readonly filters: readonly WorkbenchSearchFilterView[];
	readonly filtersLabel: string;
	readonly footerBackOrClose: string;
	readonly footerRun: string;
	readonly footerSelect: string;
	readonly isPickingArgument: boolean;
	readonly listId: string;
	readonly liveMessage: string;
	readonly onInputChange: React.ChangeEventHandler<HTMLInputElement>;
	readonly onInputKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onOpenChange: (open: boolean) => void;
	readonly pickerPrompt: string | null;
	readonly placeholder: string;
	readonly query: string;
	readonly resultCountAnnouncement: string;
	readonly resultsLabel: string;
	readonly sections: readonly WorkbenchSearchSectionView[];
}

export function useWorkbenchSearchSurface({
	platform,
	runtime,
	scope,
	state,
	update,
}: UseWorkbenchSearchSurfaceOptions): WorkbenchSearchSurfaceView {
	const intl = useOrchbeeIntl();
	const [liveMessage, setLiveMessage] = React.useState('');
	const highlightedElement = React.useRef<HTMLDivElement | null>(null);
	const catalog = React.useMemo(
		() => projectWorkbenchSearchCatalog({ intl, platform, runtime, scope }),
		[intl, platform, runtime, scope],
	);
	const browsingSections = React.useMemo(
		() =>
			state.kind === 'browsing'
				? selectWorkbenchSearchSections({
						catalog,
						filter: state.filter,
						query: state.query,
					})
				: [],
		[catalog, state],
	);
	const browsingResults = React.useMemo(
		() => browsingSections.flatMap((section) => section.results),
		[browsingSections],
	);
	const pickerAction =
		state.kind === 'picking-argument'
			? (catalog.actionResults.find((result) => result.id === state.actionResultId) ?? null)
			: null;
	const picker =
		pickerAction?.invocation.kind === 'requires-input'
			? (pickerAction.invocation.picker ?? null)
			: null;
	const pickerOptions = React.useMemo(
		() =>
			state.kind === 'picking-argument' && picker !== null
				? filterWorkbenchSearchArgumentOptions(picker.options, state.query)
				: [],
		[picker, state],
	);
	const resultIds =
		state.kind === 'browsing'
			? browsingResults.map((result) => result.id)
			: pickerOptions.map((option) => `argument:${option.id}`);
	const enabledResultIds =
		state.kind === 'browsing'
			? browsingResults.filter(isWorkbenchSearchResultEnabled).map((result) => result.id)
			: resultIds;
	const highlightedResultId = resolveWorkbenchSearchHighlight(
		state.highlightedResultId,
		resultIds,
		enabledResultIds,
	);
	const listId = `workbench-search-results-${state.sessionId}`;
	const invokeBrowsingResult = useMemoizedFn((result: WorkbenchSearchResult) => {
		const disabledReason =
			result.surfaceDisabledReason ??
			(result.availability.kind === 'disabled'
				? formatWorkbenchActionMessage(intl, result.availability.reason)
				: undefined);
		if (disabledReason !== undefined) {
			setLiveMessage(`${result.label}. ${disabledReason}`);
			return;
		}
		if (result.invocation.kind === 'requires-input') {
			if (result.invocation.picker === undefined) {
				return;
			}
			update({ actionResultId: result.id, type: 'start-argument-picker' });
			return;
		}
		update({ type: 'close' });
		runtime.executePrepared(result.invocation.prepared);
	});
	const invokePickerOption = useMemoizedFn((optionId: string) => {
		if (
			pickerAction?.invocation.kind !== 'requires-input' ||
			pickerAction.invocation.picker === undefined
		) {
			return;
		}
		const option = pickerAction.invocation.picker.options.find(
			(candidate) => candidate.id === optionId,
		);
		if (option === undefined) {
			return;
		}
		const prepared = runtime.prepareInvocation(pickerAction.invocation.unbound, option.input);
		if (prepared === null) {
			setLiveMessage(intl.formatMessage(workbenchMessages.searchActionsNeedContext));
			return;
		}
		update({ type: 'close' });
		runtime.executePrepared(prepared.invocation);
	});
	const sections = React.useMemo<readonly WorkbenchSearchSectionView[]>(() => {
		if (state.kind === 'picking-argument') {
			return picker === null
				? []
				: [
						{
							id: 'arguments',
							label: intl.formatMessage(workbenchMessages.searchArgumentOptions),
							labelId: `workbench-search-group-${state.sessionId}-arguments`,
							rows: pickerOptions.map((option) => {
								const id = `argument:${option.id}`;
								const selected = highlightedResultId === id;
								return {
									...(option.description === undefined ? {} : { description: option.description }),
									disabled: false,
									elementId: `workbench-search-option-${state.sessionId}-${id}`,
									...(selected
										? {
												elementRef: (element: HTMLDivElement | null) => {
													highlightedElement.current = element;
												},
											}
										: {}),
									icon: 'layout',
									id,
									label: option.label,
									onInvoke: () => invokePickerOption(option.id),
									onPointerDown: (event) => event.preventDefault(),
									onPointerMove: () => update({ resultId: id, type: 'highlight' }),
									selected,
									showsPicker: false,
								};
							}),
						},
					];
		}
		return browsingSections.map((section) => ({
			id: section.id,
			label: section.label,
			labelId: `workbench-search-group-${state.sessionId}-${section.id}`,
			rows: section.results.map((result) => {
				const selected = highlightedResultId === result.id;
				const disabledReason =
					result.surfaceDisabledReason ??
					(result.availability.kind === 'disabled'
						? formatWorkbenchActionMessage(intl, result.availability.reason)
						: undefined);
				return {
					...(result.description === undefined ? {} : { description: result.description }),
					disabled: !isWorkbenchSearchResultEnabled(result),
					...(disabledReason === undefined ? {} : { disabledReason }),
					elementId: `workbench-search-option-${state.sessionId}-${result.id}`,
					...(selected
						? {
								elementRef: (element: HTMLDivElement | null) => {
									highlightedElement.current = element;
								},
							}
						: {}),
					icon: result.icon,
					id: result.id,
					label: result.label,
					onInvoke: () => invokeBrowsingResult(result),
					onPointerDown: (event) => event.preventDefault(),
					onPointerMove: () => update({ resultId: result.id, type: 'highlight' }),
					selected,
					...(result.shortcut === undefined ? {} : { shortcut: result.shortcut }),
					showsPicker:
						result.invocation.kind === 'requires-input' && result.invocation.picker !== undefined,
				};
			}),
		}));
	}, [
		browsingSections,
		highlightedResultId,
		intl,
		invokeBrowsingResult,
		invokePickerOption,
		picker,
		pickerOptions,
		state,
		update,
	]);
	const rows = sections.flatMap((section) => section.rows);

	React.useEffect(() => {
		highlightedElement.current?.scrollIntoView?.({ block: 'nearest' });
	}, [highlightedResultId]);

	const filtersById = {
		actions: intl.formatMessage(workbenchMessages.searchFilterActions),
		agents: intl.formatMessage(workbenchMessages.searchFilterAgents),
		all: intl.formatMessage(workbenchMessages.searchFilterAll),
		settings: intl.formatMessage(workbenchMessages.searchFilterSettings),
	};
	const filterIds = ['all', 'agents', 'actions', 'settings'] as const;
	const filters = filterIds.map((filter, index) => ({
		id: filter,
		label: filtersById[filter],
		onKeyDown: (event: React.KeyboardEvent<HTMLButtonElement>) => {
			let nextIndex: number | null = null;
			switch (event.key) {
				case 'ArrowLeft':
					nextIndex = (index - 1 + filterIds.length) % filterIds.length;
					break;
				case 'ArrowRight':
					nextIndex = (index + 1) % filterIds.length;
					break;
				case 'Home':
					nextIndex = 0;
					break;
				case 'End':
					nextIndex = filterIds.length - 1;
					break;
			}
			if (nextIndex === null) {
				return;
			}
			event.preventDefault();
			const nextFilter = filterIds[nextIndex];
			if (nextFilter === undefined) {
				return;
			}
			update({ filter: nextFilter, type: 'filter-changed' });
			const tabs =
				event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]');
			tabs?.[nextIndex]?.focus();
		},
		onPointerDown: (event: React.PointerEvent<HTMLButtonElement>) => event.preventDefault(),
		onSelect: () => update({ filter, type: 'filter-changed' }),
		selected: state.kind === 'browsing' && state.filter === filter,
	}));
	const filter = state.kind === 'browsing' ? state.filter : state.parent.filter;
	const emptyMessages = {
		actions: intl.formatMessage(workbenchMessages.searchEmptyActions),
		agents: intl.formatMessage(workbenchMessages.searchEmptyAgents),
		all: intl.formatMessage(workbenchMessages.searchEmptyAll),
		settings: intl.formatMessage(workbenchMessages.searchEmptySettings),
	};
	const placeholders = {
		actions: intl.formatMessage(workbenchMessages.searchInputActions),
		agents: intl.formatMessage(workbenchMessages.searchInputAgents),
		all: intl.formatMessage(workbenchMessages.searchInputAll),
		settings: intl.formatMessage(workbenchMessages.searchInputSettings),
	};

	return {
		activeDescendant:
			highlightedResultId === null
				? undefined
				: rows.find((row) => row.id === highlightedResultId)?.elementId,
		dialogDescription: intl.formatMessage(workbenchMessages.searchDialogDescription),
		dialogTitle: intl.formatMessage(workbenchMessages.searchDialogTitle),
		emptyMessage: emptyMessages[filter],
		filters,
		filtersLabel: intl.formatMessage(workbenchMessages.searchFiltersLabel),
		footerBackOrClose: intl.formatMessage(
			state.kind === 'picking-argument'
				? workbenchMessages.searchFooterBack
				: workbenchMessages.searchFooterClose,
		),
		footerRun: intl.formatMessage(workbenchMessages.searchFooterRun),
		footerSelect: intl.formatMessage(workbenchMessages.searchFooterSelect),
		isPickingArgument: state.kind === 'picking-argument',
		listId,
		liveMessage,
		onInputChange: (event) => {
			setLiveMessage('');
			update({ query: event.currentTarget.value, type: 'query-changed' });
		},
		onInputKeyDown: (event) => {
			if (event.nativeEvent.isComposing) {
				return;
			}
			if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
				event.preventDefault();
				if (highlightedResultId !== null) {
					update({ resultId: highlightedResultId, type: 'highlight' });
				}
				update({
					direction: event.key === 'ArrowDown' ? 1 : -1,
					resultIds,
					type: 'move',
				});
				return;
			}
			if (event.key === 'Enter') {
				event.preventDefault();
				rows.find((row) => row.id === highlightedResultId)?.onInvoke();
				return;
			}
			if (event.key === 'Escape') {
				event.preventDefault();
				event.stopPropagation();
				update({ type: state.kind === 'picking-argument' ? 'back' : 'close' });
			}
		},
		onOpenChange: (open) => {
			if (!open) {
				update({ type: 'close' });
			}
		},
		pickerPrompt: picker?.prompt ?? null,
		placeholder:
			state.kind === 'picking-argument' && picker !== null
				? picker.placeholder
				: placeholders[filter],
		query: state.query,
		resultCountAnnouncement: intl.formatMessage(workbenchMessages.searchResultsCount, {
			count: rows.length,
		}),
		resultsLabel: intl.formatMessage(workbenchMessages.searchResultsLabel),
		sections,
	};
}
