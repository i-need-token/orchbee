import { useMemoizedFn } from '@orchbee/foundation/react';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import { Dialog, DialogContent, FileIcon, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import {
	useWorkbenchKeyboardScope,
	type WorkbenchKeyboardScope,
} from '../../../../../workbench-keyboard-scopes';
import type {
	WorkbenchQuickInputSession,
	WorkbenchQuickInputSessions,
	WorkbenchQuickPickRow,
} from '../../model/WorkbenchQuickInputSessions';
import { useWorkbenchQuickInputFocusReturn } from './hooks/useWorkbenchQuickInputFocusReturn';
import styles from './WorkbenchQuickInputSurface.module.css';

/** An open picker is the narrowest keyboard scope, so its own bindings win while it has focus. */
const QUICK_INPUT_SCOPE: WorkbenchKeyboardScope = { kind: 'quick-input' };

interface WorkbenchQuickInputSurfaceProps {
	readonly sessions: WorkbenchQuickInputSessions;
}

/** The current quick input session, placed like unified search; nothing while none is open. */
export function WorkbenchQuickInputSurface({
	sessions,
}: WorkbenchQuickInputSurfaceProps): React.ReactNode {
	const session = React.useSyncExternalStore(sessions.subscribe, sessions.getSnapshot);
	// Each session's dialog in turn, which the keys leave once its session ends.
	const surfaceReference = React.useRef<HTMLDivElement>(null);
	useWorkbenchQuickInputFocusReturn(sessions, surfaceReference);
	return session === null ? null : (
		<QuickInputSession
			key={session.id}
			session={session}
			sessions={sessions}
			surfaceReference={surfaceReference}
		/>
	);
}

function QuickInputSession({
	session,
	sessions,
	surfaceReference,
}: {
	readonly session: WorkbenchQuickInputSession;
	readonly sessions: WorkbenchQuickInputSessions;
	readonly surfaceReference: React.RefObject<HTMLDivElement | null>;
}): React.ReactNode {
	const intl = useOrchbeeIntl();
	const scopeReference = React.useRef<HTMLDivElement>(null);
	useWorkbenchKeyboardScope(scopeReference, QUICK_INPUT_SCOPE);
	const listId = React.useId();
	const [filter, setFilter] = React.useState('');
	const [activeIndex, setActiveIndex] = React.useState(0);
	const shown =
		session.kind === 'pick' ? shownRows(session.rows, filter, session.matchOnDescription) : [];
	const choices = shown.flatMap((row) => (row.kind === 'item' ? [row] : []));
	const active = choices[Math.min(activeIndex, choices.length - 1)];
	const label = session.frame.title ?? intl.formatMessage(workbenchMessages.quickInputLabel);
	const prompt = session.kind === 'input' ? session.prompt : undefined;
	const step = session.frame.step;
	const busy = session.frame.busy === true;
	const validation = session.kind === 'input' ? session.validation : null;

	const onOpenChange = useMemoizedFn((open: boolean): void => {
		if (!open && !(session.kind === 'input' && session.ignoreFocusOut)) sessions.cancel(session.id);
	});
	const onChange = useMemoizedFn((event: React.ChangeEvent<HTMLInputElement>): void => {
		if (session.kind === 'input') {
			sessions.changeValue(session.id, event.currentTarget.value);
			return;
		}
		setFilter(event.currentTarget.value);
		setActiveIndex(0);
	});
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		switch (event.key) {
			case 'Escape':
				sessions.cancel(session.id);
				break;
			case 'Enter':
				if (event.nativeEvent.isComposing) return;
				sessions.accept(session.id, active?.id);
				break;
			case 'ArrowDown':
			case 'ArrowUp': {
				if (choices.length === 0) return;
				const offset = event.key === 'ArrowDown' ? 1 : choices.length - 1;
				setActiveIndex((Math.min(activeIndex, choices.length - 1) + offset) % choices.length);
				break;
			}
			default:
				return;
		}
		event.preventDefault();
	});

	return (
		<Dialog onOpenChange={onOpenChange} open>
			<DialogContent
				aria-label={label}
				className={styles['surface']}
				data-slot='workbench-quick-input'
				finalFocus={false}
				ref={surfaceReference}
				showCloseButton={false}
			>
				<div className={styles['scope']} ref={scopeReference}>
					<div className={styles['inputRegion']}>
						<ProductIcon name='search' />
						<input
							aria-activedescendant={active === undefined ? undefined : `${listId}-${active.id}`}
							aria-controls={session.kind === 'pick' ? listId : undefined}
							aria-expanded={session.kind === 'pick' ? true : undefined}
							aria-invalid={validation?.severity === 'error' || undefined}
							aria-label={label}
							autoComplete='off'
							autoFocus
							className={styles['input']}
							placeholder={session.frame.placeholder}
							role={session.kind === 'pick' ? 'combobox' : undefined}
							spellCheck={false}
							type={session.kind === 'input' && session.password ? 'password' : 'text'}
							value={session.kind === 'input' ? session.value : filter}
							onChange={onChange}
							onKeyDown={onKeyDown}
						/>
					</div>
					{prompt === undefined && step === undefined && !busy ? null : (
						<div aria-live='polite' className={styles['frameLine']}>
							{prompt === undefined ? null : <span>{prompt}</span>}
							{step === undefined ? null : (
								<span>{intl.formatMessage(workbenchMessages.quickInputStep, step)}</span>
							)}
							{busy ? <span>{intl.formatMessage(workbenchMessages.quickInputBusy)}</span> : null}
						</div>
					)}
					{validation === null ? null : (
						<div
							className={styles['message']}
							data-severity={validation.severity}
							role={validation.severity === 'error' ? 'alert' : 'status'}
						>
							{validation.message}
						</div>
					)}
					{session.kind === 'pick' ? (
						<div aria-label={label} className={styles['results']} id={listId} role='listbox'>
							{choices.length === 0 ? (
								<div className={styles['empty']}>
									{intl.formatMessage(workbenchMessages.quickInputNoMatches)}
								</div>
							) : (
								shown.map((row, index) =>
									row.kind === 'separator' ? (
										<div
											className={styles['sectionLabel']}
											key={`separator-${String(index)}`}
											role='presentation'
										>
											{row.label}
										</div>
									) : (
										<div
											aria-selected={row.id === active?.id}
											className={styles['row']}
											data-highlighted={row.id === active?.id}
											id={`${listId}-${row.id}`}
											key={row.id}
											role='option'
											onClick={() => sessions.accept(session.id, row.id)}
											onPointerDown={(event) => event.preventDefault()}
										>
											<span className={styles['rowIcon']}>
												{row.icon === undefined ? null : (
													<FileIcon name={row.icon.name} path={row.icon.path} />
												)}
											</span>
											<span
												className={styles['rowContent']}
												data-italic={row.italic ? '' : undefined}
											>
												<span className={styles['rowLabel']}>{row.label}</span>
												{row.description === undefined ? null : (
													<span className={styles['rowDescription']}>{row.description}</span>
												)}
											</span>
										</div>
									),
								)
							)}
						</div>
					) : null}
				</div>
			</DialogContent>
		</Dialog>
	);
}

/**
 * The rows a filter keeps: items whose label, or description when the pick matches on it, holds the
 * text typed, and separators only while nothing is typed.
 */
function shownRows(
	rows: readonly WorkbenchQuickPickRow[],
	filter: string,
	matchOnDescription: boolean,
): readonly WorkbenchQuickPickRow[] {
	const needle = filter.trim().toLocaleLowerCase();
	if (needle === '') return rows;
	const matches = (text: string | undefined): boolean =>
		text !== undefined && text.toLocaleLowerCase().includes(needle);
	return rows.filter(
		(row) =>
			row.kind === 'item' &&
			(matches(row.label) || (matchOnDescription && matches(row.description))),
	);
}
