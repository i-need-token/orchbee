import {
	Button,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Input,
	Kbd,
	NativeSelect,
	NativeSelectOption,
} from '@orchbee/ui/components';
import type * as React from 'react';

import type {
	WorkbenchAgentCatalogSource,
	WorkbenchAgentStartInput,
	WorkbenchAgentStartPlace,
} from '../../types/workbench-agent-start';
import { WorkbenchAgentProviderIcon } from '../workbench-agent-provider-icon/WorkbenchAgentProviderIcon';
import {
	useWorkbenchAgentStartDialog,
	type WorkbenchAgentStartProviderRowView,
} from './hooks/useWorkbenchAgentStartDialog';
import styles from './WorkbenchAgentStartDialog.module.css';

interface WorkbenchAgentStartDialogProps {
	readonly catalog: WorkbenchAgentCatalogSource;
	readonly finalFocus: () => HTMLElement | false | null;
	readonly onClose: () => void;
	readonly onStart: (input: WorkbenchAgentStartInput) => void;
	/** The places a start can use now; the first is the default. */
	readonly places: readonly WorkbenchAgentStartPlace[];
}

/** Start Agent: the Session's Agents to start, one per row, and where the started Agent opens. */
export function WorkbenchAgentStartDialog({
	catalog,
	finalFocus,
	onClose,
	onStart,
	places,
}: WorkbenchAgentStartDialogProps): React.ReactNode {
	const view = useWorkbenchAgentStartDialog({ catalog, onStart, places });
	const option = (row: WorkbenchAgentStartProviderRowView): React.ReactNode => (
		<div
			aria-selected={row.highlighted}
			className={styles['row']}
			data-highlighted={row.highlighted}
			id={row.elementId}
			key={row.providerId}
			ref={row.elementRef}
			role='option'
			onClick={row.onClick}
			onDoubleClick={row.onDoubleClick}
		>
			<span aria-hidden='true' className={styles['icon']}>
				<WorkbenchAgentProviderIcon
					agents={[row.providerId]}
					gradientPrefix={`${view.gradientPrefix}-${row.providerId}`}
				/>
			</span>
			<span className={styles['name']}>{row.name}</span>
			<span className={styles['detail']}>{row.detail}</span>
		</div>
	);

	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent
				className={styles['surface']}
				closeLabel={view.closeLabel}
				data-slot='workbench-agent-start'
				finalFocus={finalFocus}
				initialFocus={view.searchRef}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{view.title}</DialogTitle>
				</DialogHeader>
				<Input
					aria-activedescendant={view.activeDescendant}
					aria-autocomplete='list'
					aria-controls={view.listId}
					aria-expanded='true'
					aria-label={view.searchLabel}
					autoCapitalize='none'
					autoComplete='off'
					autoCorrect='off'
					className={styles['search']}
					data-slot='workbench-agent-start-search'
					placeholder={view.searchLabel}
					ref={view.searchRef}
					role='combobox'
					spellCheck={false}
					value={view.query}
					onChange={view.onQueryChange}
					onKeyDown={view.onSearchKeyDown}
				/>
				{/* The search field keeps the keyboard; a click only moves the highlight. */}
				<div
					aria-label={view.listLabel}
					className={styles['list']}
					data-slot='workbench-agent-start-list'
					id={view.listId}
					role='listbox'
					onMouseDown={(event) => event.preventDefault()}
				>
					{view.sections.map((section) =>
						section.kind === 'fold' ? (
							<div
								aria-selected={section.highlighted}
								className={styles['row']}
								data-highlighted={section.highlighted}
								data-slot='workbench-agent-start-fold'
								id={section.elementId}
								key={section.key}
								ref={section.elementRef}
								role='option'
								onClick={section.onClick}
							>
								<span className={styles['fold']}>{section.label}</span>
							</div>
						) : (
							<div
								aria-labelledby={section.headingId}
								className={styles['group']}
								key={section.key}
								role='group'
							>
								<div className={styles['heading']} id={section.headingId} role='presentation'>
									{section.label}
								</div>
								{section.rows.map(option)}
							</div>
						),
					)}
				</div>
				<p
					aria-live='polite'
					className={styles['statusLine']}
					data-slot='workbench-agent-start-status'
					data-warning={view.status?.warning === true}
				>
					{view.status?.message ?? '\u00A0'}
				</p>
				<DialogFooter className={styles['footer']}>
					<span className={styles['place']}>
						<label className={styles['placeLabel']} htmlFor={view.placeId}>
							{view.opensInLabel}
						</label>
						<NativeSelect
							disabled={view.places.length === 0}
							id={view.placeId}
							size='sm'
							value={view.place}
							onChange={view.onPlaceChange}
						>
							{view.places.map((place) => (
								<NativeSelectOption key={place.value} value={place.value}>
									{place.label}
								</NativeSelectOption>
							))}
						</NativeSelect>
					</span>
					<Button className={styles['action']} type='button' variant='outline' onClick={onClose}>
						{view.cancelLabel}
					</Button>
					<Button
						className={styles['action']}
						disabled={!view.canStart}
						type='button'
						onClick={view.start}
					>
						{view.startLabel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
