import {
	Button,
	Checkbox,
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	Input,
	Kbd,
} from '@orchbee/ui/components';
import * as React from 'react';

import { useTerminalSettingsMachines } from '../../hooks/useTerminalSettingsMachines';
import type {
	TerminalSettingsMachineLogin,
	TerminalSettingsMachines,
} from '../../types/terminal-settings-surface';
import styles from './MachineLoginDialog.module.css';

/** The word a new host's field must read, which it shows in clear while typed. */
const YES = 'yes';

interface MachineLoginDialogProps {
	readonly machines: TerminalSettingsMachines;
}

/**
 * The login question of the machine the person waits on, as the TUI's overlay asks it. A login
 * that ended without the person keeps the Dialog, without the field, until Close.
 */
export function MachineLoginDialog({ machines }: MachineLoginDialogProps): React.ReactNode {
	const login = useTerminalSettingsMachines(machines)?.login ?? null;
	if (login === null) return null;
	// Each question starts with an empty field and the box as the login left it.
	return (
		<MachineLoginQuestion
			key={`${login.login}:${login.question}`}
			login={login}
			request={machines.request}
		/>
	);
}

interface MachineLoginQuestionProps {
	readonly login: TerminalSettingsMachineLogin;
	readonly request: TerminalSettingsMachines['request'];
}

function MachineLoginQuestion({ login, request }: MachineLoginQuestionProps): React.ReactNode {
	const formId = React.useId();
	const fieldId = React.useId();
	const statusId = React.useId();
	const field = React.useRef<HTMLInputElement>(null);
	const [answer, setAnswer] = React.useState('');
	const [ticked, setTicked] = React.useState(login.remember?.ticked ?? false);
	const [typeYes, setTypeYes] = React.useState(false);
	const ids = { login: login.login, question: login.question };
	const hostKey = login.kind === 'host-key';
	const ended = login.ended !== null;
	const close = (): void => {
		setAnswer('');
		request({ ...ids, act: ended ? 'login-close' : 'login-cancel' });
	};
	const submit = (): void => {
		if (ended) {
			close();
			return;
		}
		if (login.sending) return;
		if (hostKey) {
			if (answer.trim().toLowerCase() === YES) {
				setAnswer('');
				request({ ...ids, act: 'login-trust' });
			} else {
				setTypeYes(true);
			}
			return;
		}
		// An empty password or passphrase would cost an attempt.
		if (answer === '' && login.kind !== 'other') return;
		const typed = answer;
		setAnswer('');
		request({
			...ids,
			act: 'login-answer',
			answer: typed,
			remember: login.remember === null ? null : ticked,
		});
	};
	let status: {
		readonly text: string;
		readonly tone: 'error' | 'help' | 'pending' | 'success';
	} | null = null;
	if (login.ended !== null) {
		status = { text: login.ended.text, tone: login.ended.succeeded ? 'success' : 'error' };
	} else if (login.sending) {
		status = { text: login.labels.sending, tone: 'pending' };
	} else if (typeYes) {
		status = { text: login.labels.typeYes, tone: 'error' };
	} else if (login.notice !== null) {
		status = { text: login.notice, tone: 'error' };
	} else if (login.remember !== null && ticked) {
		status = { text: login.labels.rememberPrivacy, tone: 'help' };
	} else if (login.labels.privacy !== null) {
		status = { text: login.labels.privacy, tone: 'help' };
	}
	// A new host's field shows what is typed while it begins `yes`; anything else is masked.
	const shownClear = hostKey && YES.startsWith(answer.toLowerCase());
	return (
		<Dialog
			open
			onOpenChange={(open) => {
				if (!open) close();
			}}
		>
			<DialogContent
				closeLabel={login.labels.close}
				data-slot='machine-login-dialog'
				initialFocus={ended ? undefined : field}
			>
				<DialogHeader className={styles['header']}>
					<DialogTitle className={styles['title']}>{login.labels.title}</DialogTitle>
				</DialogHeader>
				<form
					className={styles['body']}
					id={formId}
					onSubmit={(event) => {
						event.preventDefault();
						submit();
					}}
				>
					<p className={styles['machine']}>{login.machine}</p>
					{ended ? null : (
						<>
							{login.said.length > 0 ? (
								<p className={styles['said']}>{login.said.join('\n')}</p>
							) : null}
							<div className={styles['asked']}>
								{login.asked.map((line, index) => (
									<label
										key={line}
										className={hostKey && index === 1 ? styles['fingerprint'] : styles['askedLine']}
										htmlFor={fieldId}
									>
										{line}
									</label>
								))}
							</div>
							<Input
								ref={field}
								aria-describedby={status === null ? undefined : statusId}
								aria-invalid={typeYes || login.notice !== null || undefined}
								autoComplete='off'
								disabled={login.sending}
								id={fieldId}
								spellCheck={false}
								type={shownClear ? 'text' : 'password'}
								value={answer}
								onChange={(event) => {
									setAnswer(event.currentTarget.value);
									setTypeYes(false);
								}}
							/>
							{login.remember === null ? null : (
								<label className={styles['remember']}>
									<Checkbox
										checked={ticked}
										disabled={login.sending}
										onCheckedChange={(checked) => setTicked(checked)}
									/>
									{login.remember.label}
								</label>
							)}
							{login.rememberLine === null ? null : (
								<p className={styles['help']}>{login.rememberLine}</p>
							)}
						</>
					)}
					{status === null ? null : (
						<p
							className={styles[status.tone]}
							id={statusId}
							role={status.tone === 'error' ? 'alert' : undefined}
						>
							{status.text}
						</p>
					)}
				</form>
				<DialogFooter>
					{ended ? null : (
						<Button className={styles['action']} type='button' variant='outline' onClick={close}>
							{login.labels.cancel}
							<Kbd aria-hidden='true' className={styles['shortcut']}>
								Esc
							</Kbd>
						</Button>
					)}
					<Button
						className={styles['action']}
						disabled={!ended && login.sending}
						form={formId}
						type='submit'
					>
						{ended ? login.labels.close : login.labels.submit}
						<Kbd aria-hidden='true' className={styles['shortcut']}>
							Enter
						</Kbd>
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
