import { Dialog, DialogContent, Field, FieldDescription, Input } from '@orchbee/ui/components';
import { WORKBENCH_MAXIMUM_LABEL_LENGTH } from '@orchbee/workbench-client-protocol';
import { useId, useRef, useState, type ReactNode } from 'react';

import { ResourceDialogForm } from '../resource-dialog-form/ResourceDialogForm';
import { ResourceDialogFooter } from '../resource-dialog-footer/ResourceDialogFooter';
import { ResourceDialogHeader } from '../resource-dialog-header/ResourceDialogHeader';
import styles from './NameDialog.module.css';

interface NameDialogMessages {
	readonly cancel: string;
	readonly close: string;
	readonly nameTooLong: string;
}

/** A blank name is refused with a reason, or accepted with help that says what it does. */
export type NameDialogBlankRule =
	| { readonly kind: 'refuse'; readonly reason: string }
	| { readonly help: string; readonly kind: 'accept' };

interface NameDialogProps {
	readonly blank: NameDialogBlankRule;
	readonly finalFocus?: (() => HTMLElement | null) | undefined;
	readonly messages: NameDialogMessages;
	readonly name: string;
	readonly onClose: () => void;
	readonly onNameChange: (name: string) => void;
	readonly onSubmit: (name: string) => void;
	/** Shown while the field is empty, such as an automatic name that stays until one is typed. */
	readonly placeholder?: string | undefined;
	readonly submitLabel: string;
	/** Names the only field too, so the field has no label of its own. */
	readonly title: string;
}

/** Why the name is refused, or null when it is not. */
function refusal(name: string, blank: NameDialogBlankRule, tooLong: string): string | null {
	if (blank.kind === 'refuse' && !/[^\p{White_Space}]/u.test(name)) return blank.reason;
	return [...name].length > WORKBENCH_MAXIMUM_LABEL_LENGTH ? tooLong : null;
}

/**
 * Asks for one name, as the TUI's name dialogs do: the title names the field, the name is kept
 * exactly as typed, and a refused name says why when submitted or left, the reason then following
 * the field until the name is fixed. Help about a blank name shows while nothing is wrong.
 */
export function NameDialog({
	blank,
	finalFocus,
	messages,
	name,
	onClose,
	onNameChange,
	onSubmit,
	placeholder,
	submitLabel,
	title,
}: NameDialogProps): ReactNode {
	const descriptionId = useId();
	const formId = useId();
	const titleId = useId();
	// Not `autoFocus`: the menu this opens from returns focus as it closes, after that has run.
	const input = useRef<HTMLInputElement>(null);
	const [validated, setValidated] = useState(false);
	const error = refusal(name, blank, messages.nameTooLong);
	const shownError = validated ? error : null;
	let description: ReactNode = null;
	if (shownError !== null) {
		description = (
			<p className={styles['error']} id={descriptionId} role='alert'>
				{shownError}
			</p>
		);
	} else if (blank.kind === 'accept') {
		description = <FieldDescription id={descriptionId}>{blank.help}</FieldDescription>;
	}
	return (
		<Dialog
			onOpenChange={(open) => {
				if (!open) onClose();
			}}
			open
		>
			<DialogContent
				closeLabel={messages.close}
				data-slot='workbench-name-dialog'
				finalFocus={finalFocus}
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
							aria-describedby={description === null ? undefined : descriptionId}
							aria-invalid={shownError !== null || undefined}
							aria-labelledby={titleId}
							aria-required={blank.kind === 'refuse' || undefined}
							placeholder={placeholder}
							ref={input}
							value={name}
							onBlur={() => setValidated(true)}
							onChange={(event) => onNameChange(event.currentTarget.value)}
							onFocus={(event) => event.currentTarget.select()}
						/>
						{description}
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
