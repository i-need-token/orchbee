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
import { MachineColorChooser } from '../machine-color-chooser/MachineColorChooser';
import styles from './MachineAddDialog.module.css';

interface MachineAddDialogProps {
	readonly add: TerminalSettingsMachinesView['add'];
	readonly machines: TerminalSettingsMachines;
	/** The form closed: given up, or its machine saved. */
	readonly onClose: () => void;
}

interface Submitted {
	readonly address: string;
	readonly name: string;
}

/**
 * Add Machine, as the TUI's form adds: what follows `ssh` or a host from the SSH config, an optional
 * name and the color offered first. Add runs the eight checks and names the one running; a login
 * question opens over the form. A failed check names itself and gives the add up; editing the
 * address or name asks again. Once every check passed the machine is saved and the form closes.
 */
export function MachineAddDialog({
	add,
	machines,
	onClose,
}: MachineAddDialogProps): React.ReactNode {
	const formId = React.useId();
	const addressId = React.useId();
	const addressNoteId = React.useId();
	const nameId = React.useId();
	const nameNoteId = React.useId();
	const statusId = React.useId();
	const addressField = React.useRef<HTMLInputElement>(null);
	const { form, labels } = add;
	const [address, setAddress] = React.useState('');
	const [name, setName] = React.useState('');
	const [color, setColor] = React.useState<TerminalSettingsMachineColor | null>(null);
	const [submitted, setSubmitted] = React.useState<Submitted | null>(null);
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
	// What the process reports describes the address and name last added, not ones edited since.
	const current =
		form !== null && submitted !== null && submitted.address === address && submitted.name === name;
	const status = current ? form.status : null;
	const lines = current ? form.lines : [];
	const problem = current ? form.problem : null;
	const blocked = current && form.blocked;
	const chosen = color ?? form?.color ?? 'cyan';
	const submit = (): void => {
		if (running || blocked) return;
		setSubmitted({ address, name });
		machines.request({ act: 'add', address, color: chosen, name });
	};
	const addressProblem = problem?.field === 'address' ? problem.text : null;
	const nameProblem = problem?.field === 'name' ? problem.text : null;
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
						<Input
							ref={addressField}
							aria-describedby={addressProblem === null ? undefined : addressNoteId}
							aria-invalid={addressProblem !== null || undefined}
							autoComplete='off'
							disabled={running}
							id={addressId}
							placeholder='user@10.0.0.8'
							spellCheck={false}
							value={address}
							onChange={(event) => setAddress(event.currentTarget.value)}
						/>
						{addressProblem === null ? null : (
							<p className={styles['error']} id={addressNoteId} role='alert'>
								{addressProblem}
							</p>
						)}
						{form === null || form.hosts.length === 0 ? null : (
							<div className={styles['hosts']}>
								<span className={styles['hostsLabel']}>{labels.hosts}</span>
								{form.hosts.map((entry) => (
									<Button
										key={entry.host}
										disabled={running}
										size='sm'
										type='button'
										variant='ghost'
										onClick={() => {
											setAddress(entry.host);
											addressField.current?.focus();
										}}
									>
										{entry.label}
									</Button>
								))}
							</div>
						)}
					</div>
					<div className={styles['field']}>
						<label className={styles['label']} htmlFor={nameId}>
							{labels.name}
						</label>
						<Input
							aria-describedby={nameNoteId}
							aria-invalid={nameProblem !== null || undefined}
							autoComplete='off'
							disabled={running}
							id={nameId}
							spellCheck={false}
							value={name}
							onChange={(event) => setName(event.currentTarget.value)}
						/>
						<p
							className={nameProblem === null ? styles['help'] : styles['error']}
							id={nameNoteId}
							role={nameProblem === null ? undefined : 'alert'}
						>
							{nameProblem ?? labels.nameHint}
						</p>
					</div>
					<div className={styles['color']}>
						<span className={styles['label']}>{labels.color}</span>
						<MachineColorChooser
							color={chosen}
							disabled={running}
							label={labels.color}
							onChoose={setColor}
						/>
					</div>
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
