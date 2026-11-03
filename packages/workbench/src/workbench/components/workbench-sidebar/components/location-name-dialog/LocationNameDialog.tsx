import { Dialog, DialogContent, Field, Input } from '@orchbee/ui/components';
import { WORKBENCH_MAXIMUM_LABEL_LENGTH } from '@orchbee/workbench-client-protocol';
import { useId, useRef, useState, type ReactNode } from 'react';

import { ResourceDialogForm } from '../../../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../../../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../../../resource-dialog-header/ResourceDialogHeader';
import styles from './LocationNameDialog.module.css';

interface LocationNameDialogMessages {
	readonly cancel: string;
	readonly close: string;
	readonly nameRequired: string;
	readonly nameTooLong: string;
}

interface LocationNameDialogProps {
	readonly messages: LocationNameDialogMessages;
	readonly name: string;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onSubmit: (name: string) => void;
	readonly submitLabel: string;
	/** Names the only field too, so the field has no label of its own. */
	readonly title: string;
}

function nameError(name: string): 'required' | 'too-long' | null {
	if (!/[^\p{White_Space}]/u.test(name)) return 'required';
	return [...name].length > WORKBENCH_MAXIMUM_LABEL_LENGTH ? 'too-long' : null;
}

/**
 * Asks for one Project or Workspace name, as the TUI's rename editor does: the name is kept exactly
 * as typed. A blank or over-long name is refused when submitted or left, saying why, and the reason
 * then follows the field until the name is fixed.
 */
export function LocationNameDialog({
	messages,
	name,
	onClose,
	onNameChange,
	onSubmit,
	submitLabel,
	title,
}: LocationNameDialogProps): ReactNode {
	const errorId = useId();
	const formId = useId();
	const titleId = useId();
	// Not `autoFocus`: the menu this opens from returns focus as it closes, after that has run.
	const input = useRef<HTMLInputElement>(null);
	const [validated, setValidated] = useState(false);
	const error = nameError(name);
	const shownError = validated ? error : null;
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent
				closeLabel={messages.close}
				data-slot='workbench-location-name-dialog'
				initialFocus={input}
			>
				<ResourceDialogHeader title={title} titleId={titleId} />
				<ResourceDialogForm
					id={formId}
					onSubmit={() => {
						if (error === null) {
							onSubmit(name);
							return;
						}
						setValidated(true);
						input.current?.focus();
					}}
					submitDisabled={false}
				>
					<Field>
						{/* Not `required`: the browser would refuse a blank name before saying why here. */}
						<Input
							aria-describedby={shownError === null ? undefined : errorId}
							aria-invalid={shownError !== null || undefined}
							aria-labelledby={titleId}
							aria-required
							ref={input}
							value={name}
							onBlur={() => setValidated(true)}
							onChange={(event) => onNameChange(event.currentTarget.value)}
							onFocus={(event) => event.currentTarget.select()}
						/>
						{shownError === null ? null : (
							<p className={styles['error']} id={errorId} role='alert'>
								{shownError === 'required' ? messages.nameRequired : messages.nameTooLong}
							</p>
						)}
					</Field>
				</ResourceDialogForm>
				<ResourceDialogFooter
					cancelLabel={messages.cancel}
					formId={formId}
					onCancel={onClose}
					submitDisabled={false}
					submitLabel={submitLabel}
				/>
			</DialogContent>
		</Dialog>
	);
}
