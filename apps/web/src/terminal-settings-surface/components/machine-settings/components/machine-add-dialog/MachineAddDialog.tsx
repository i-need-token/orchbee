import {
	Button,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Input,
	Kbd,
} from '@orchbee/ui/components';
import * as React from 'react';

import type {
	TerminalSettingsMachineColor,
	TerminalSettingsMachines,
	TerminalSettingsMachinesView,
} from '../../../../types/terminal-settings-surface';
import { MachineColorSelect } from './components/machine-color-select/MachineColorSelect';
import styles from './MachineAddDialog.module.css';

interface MachineAddDialogProps {
	readonly add: TerminalSettingsMachinesView['add'];
	readonly machines: TerminalSettingsMachines;
	/** The form closed: given up, or its machine saved. */
	readonly onClose: () => void;
}

/** Scrolls the highlighted host into the list's view. */
function reveal(element: HTMLDivElement | null): void {
	element?.scrollIntoView({ block: 'nearest' });
}

/**
 * Add Machine, as the TUI's form adds: the color before the address on one line, then the SSH
 * config hosts, one a line in a list that scrolls, any of which the address takes. No name is
 * asked: the machine takes its host's, and its page renames it. Add runs the eight checks and names
 * the one running; a login question opens over the form. A failed check names itself and gives the
 * add up; editing the address asks again. Once every check passed the machine is saved and the form
 * closes.
 */
export function MachineAddDialog({
	add,
	machines,
	onClose,
}: MachineAddDialogProps): React.ReactNode {
	const formId = React.useId();
	const addressId = React.useId();
	const addressNoteId = React.useId();
	const hostsId = React.useId();
	const hostsLabelId = React.useId();
	const statusId = React.useId();
	const addressField = React.useRef<HTMLInputElement>(null);
	const { form, labels } = add;
	const [address, setAddress] = React.useState('');
	const [color, setColor] = React.useState<TerminalSettingsMachineColor | null>(null);
	// The host Down and Up move to while the address keeps the keyboard, as the TUI's form does.
	const [highlight, setHighlight] = React.useState<number | null>(null);
	const [submitted, setSubmitted] = React.useState<string | null>(null);
	// The form lives in the machines process while the page shows it.
	React.useEffect(() => {
		machines.request({ act: 'add-open' });
		return () => machines.request({ act: 'add-close' });
	}, [machines]);
	const saved = form?.saved ?? null;
	React.useEffect(() => {
		if (saved !== null) onClose();
	}, [onClose, saved]);
	const running = form?.running ?? false;
	const hosts = form?.hosts ?? [];
	// What the process reports describes the address last added, not one edited since.
	const current = form !== null && submitted !== null && submitted === address;
	const status = current ? form.status : null;
	const lines = current ? form.lines : [];
	const problem = current ? form.problem : null;
	const blocked = current && form.blocked;
	const chosen = color ?? form?.color ?? 'cyan';
	const highlighted = highlight === null ? undefined : hosts[highlight];
	const optionId = (index: number): string => `${hostsId}-${index}`;
	const take = (host: string): void => {
		setAddress(host);
		setHighlight(null);
		addressField.current?.focus();
	};
	const submit = (): void => {
		if (running || blocked) return;
		setSubmitted(address);
		machines.request({ act: 'add', address, color: chosen });
	};
	const addressProblem = problem?.field === 'address' ? problem.text : null;
	const formProblem = problem !== null && problem.field === null ? problem.text : null;
	return (
		<Dialog
			open
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
		>
			<DialogContent
				closeLabel={labels.cancel}
				data-slot='machine-add-dialog'
				initialFocus={addressField}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{labels.title}</DialogTitle>
				</DialogHeader>
				<form
					className={styles['body']}
					id={formId}
					onSubmit={(event) => {
						event.preventDefault();
						submit();
					}}
				>
					<div className={styles['field']}>
						<label className={styles['label']} htmlFor={addressId}>
							{labels.address}
						</label>
						<div className={styles['target']}>
							<MachineColorSelect color={chosen} disabled={running} onChoose={setColor} />
							<Input
								ref={addressField}
								aria-activedescendant={highlight === null ? undefined : optionId(highlight)}
								aria-autocomplete='list'
								aria-controls={hosts.length === 0 ? undefined : hostsId}
								aria-describedby={addressProblem === null ? undefined : addressNoteId}
								aria-expanded={hosts.length > 0}
								aria-invalid={addressProblem !== null || undefined}
								autoComplete='off'
								disabled={running}
								id={addressId}
								placeholder='user@10.0.0.8'
								role='combobox'
								spellCheck={false}
								value={address}
								onChange={(event) => {
									setAddress(event.currentTarget.value);
									setHighlight(null);
								}}
								onKeyDown={(event) => {
									if (event.nativeEvent.isComposing || hosts.length === 0) return;
									if (event.key === 'ArrowDown') {
										event.preventDefault();
										setHighlight((at) => (at === null ? 0 : Math.min(at + 1, hosts.length - 1)));
									} else if (event.key === 'ArrowUp') {
										event.preventDefault();
										setHighlight((at) => (at === null || at === 0 ? null : at - 1));
									} else if (event.key === 'Enter' && highlighted !== undefined) {
										// Enter takes the highlighted host; the next Enter adds it.
										event.preventDefault();
										take(highlighted.host);
									}
								}}
							/>
						</div>
						{addressProblem === null ? null : (
							<p className={styles['error']} id={addressNoteId} role='alert'>
								{addressProblem}
							</p>
						)}
					</div>
					{hosts.length === 0 ? null : (
						<div className={styles['field']}>
							<span className={styles['label']} id={hostsLabelId}>
								{labels.hosts}
							</span>
							{/* The address keeps the keyboard; a click takes the host into it. */}
							<div
								aria-labelledby={hostsLabelId}
								className={styles['hosts']}
								id={hostsId}
								role='listbox'
								onMouseDown={(event) => event.preventDefault()}
							>
								{hosts.map((entry, index) => (
									<div
										key={entry.host}
										ref={index === highlight ? reveal : undefined}
										aria-disabled={running || undefined}
										aria-selected={index === highlight}
										className={styles['host']}
										data-highlighted={index === highlight}
										id={optionId(index)}
										role='option'
										onClick={() => {
											if (!running) take(entry.host);
										}}
									>
										{entry.label}
									</div>
								))}
							</div>
						</div>
					)}
					{status === null && formProblem === null ? null : (
						<div aria-live='polite' className={styles['status']} id={statusId}>
							{formProblem === null ? null : <p className={styles['error']}>{formProblem}</p>}
							{status === null ? null : <p className={styles[status.tone]}>{status.text}</p>}
							{lines.map((line) => (
								<p key={line} className={styles['help']}>
									{line}
								</p>
							))}
						</div>
					)}
				</form>
				<DialogFooter>
					<Button className={styles['action']} type='button' variant='outline' onClick={onClose}>
						{labels.cancel}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Esc
						</Kbd>
					</Button>
					<Button
						className={styles['action']}
						disabled={running || blocked}
						form={formId}
						type='submit'
					>
						{current && form !== null ? form.submit : labels.add}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
