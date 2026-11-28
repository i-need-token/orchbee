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
	const shown = session.kind === 'pick' ? session.rows : [];
	const activeId = session.kind === 'pick' ? session.activeId : null;
	const hasChoices = shown.some((row) => row.kind === 'item');
	const label = session.frame.title ?? intl.formatMessage(workbenchMessages.quickInputLabel);
	const prompt = session.kind === 'input' ? session.prompt : undefined;
	const step = session.frame.step;
	const busy = session.frame.busy === true;
	const validation = session.kind === 'input' ? session.validation : null;

	const onOpenChange = useMemoizedFn((open: boolean): void => {
		if (!open && !(session.kind === 'input' && session.ignoreFocusOut)) sessions.cancel(session.id);
	});
	const onChange = useMemoizedFn((event: React.ChangeEvent<HTMLInputElement>): void => {
		if (session.kind === 'input') sessions.changeValue(session.id, event.currentTarget.value);
		else sessions.changeFilter(session.id, event.currentTarget.value);
	});
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		switch (event.key) {
			case 'Escape':
				sessions.cancel(session.id);
				break;
			case 'Enter':
				if (event.nativeEvent.isComposing) return;
				sessions.accept(session.id);
				break;
			case 'ArrowDown':
			case 'ArrowUp':
				if (session.kind !== 'pick') return;
				sessions.moveActive(session.id, event.key === 'ArrowDown' ? 'next' : 'previous');
				break;
			default:
				return;
		}
		event.preventDefault();
	});
	// Letting go of the keys that opened a pick in quick navigate accepts its active item.
	const onKeyUp = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		if (session.kind !== 'pick') return;
		const { altKey, ctrlKey, key, metaKey } = event;
		sessions.keyUp(session.id, { altKey, ctrlKey, key, metaKey });
	});
	const inputHidden = session.kind === 'pick' && session.inputHidden;

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
					<div className={styles['inputRegion']} data-hidden={inputHidden ? '' : undefined}>
						<ProductIcon name='search' />
						<input
							aria-activedescendant={activeId === null ? undefined : `${listId}-${activeId}`}
							aria-controls={session.kind === 'pick' ? listId : undefined}
							aria-expanded={session.kind === 'pick' ? true : undefined}
							aria-invalid={validation?.severity === 'error' || undefined}
							aria-label={label}
							autoComplete='off'
							autoFocus
							className={styles['input']}
							placeholder={session.frame.placeholder}
							readOnly={inputHidden}
							role={session.kind === 'pick' ? 'combobox' : undefined}
							spellCheck={false}
							type={session.kind === 'input' && session.password ? 'password' : 'text'}
							value={session.kind === 'input' ? session.value : session.filter}
							onChange={onChange}
							onKeyDown={onKeyDown}
							onKeyUp={onKeyUp}
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
							{!hasChoices ? (
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
											aria-selected={row.id === activeId}
											className={styles['row']}
											data-highlighted={row.id === activeId}
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
