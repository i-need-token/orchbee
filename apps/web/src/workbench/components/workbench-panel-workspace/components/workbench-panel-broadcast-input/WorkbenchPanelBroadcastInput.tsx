import { Button, Input, Kbd } from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import type { WorkbenchPanelWorkspaceView } from '../../types/panel-workspace-view';
import { useWorkbenchPanelBroadcastInputView } from './hooks/useWorkbenchPanelBroadcastInputView';
import styles from './WorkbenchPanelBroadcastInput.module.css';

interface WorkbenchPanelBroadcastInputProps {
	readonly view: WorkbenchPanelWorkspaceView;
}

export function WorkbenchPanelBroadcastInput({
	view,
}: WorkbenchPanelBroadcastInputProps): ReactNode {
	const state = view.broadcastInput;
	const inputView = useWorkbenchPanelBroadcastInputView({
		end: view.endBroadcastInput,
		enqueue: view.enqueueBroadcastInput,
		toggleVisibility: view.toggleBroadcastInputVisibility,
	});
	if (state.kind === 'idle') return null;

	if (state.kind === 'stopped') {
		let message = view.messages.broadcastDeliveryFailed;
		if (state.reason === 'target-invalidated') {
			message = view.messages.broadcastTargetInvalidated;
		} else if (state.reason === 'partial-delivery') {
			message = view.messages.broadcastPartialDelivery(
				state.deliveredTargetCount,
				state.targetCount,
			);
		}
		return (
			<div
				className={styles['root']}
				data-broadcast-state='stopped'
				data-slot='workbench-panel-broadcast-input'
				role='alert'
			>
				<p className={styles['warning']}>{message}</p>
				<Button
					autoFocus
					onClick={view.dismissBroadcastInput}
					size='sm'
					type='button'
					variant='outline'
				>
					{view.messages.broadcastDismiss}
				</Button>
			</div>
		);
	}

	const targetCount = state.snapshot.targets.length;

	return (
		<div
			aria-label={view.messages.broadcastInputLabel}
			className={styles['root']}
			data-broadcast-state='active'
			data-input-hidden={state.hidden || undefined}
			data-slot='workbench-panel-broadcast-input'
			role='region'
		>
			<div className={styles['identity']}>
				<strong>{view.messages.broadcastInputLabel}</strong>
				<span>{view.messages.broadcastTargetCount(targetCount)}</span>
			</div>
			<Input
				aria-label={view.messages.broadcastInputLabel}
				autoCapitalize='off'
				autoComplete='off'
				autoCorrect='off'
				autoFocus
				className={styles['input']}
				data-slot='workbench-panel-broadcast-input-field'
				onBeforeInput={inputView.onBeforeInput}
				onBlur={inputView.onBlur}
				onCompositionEnd={inputView.onCompositionEnd}
				onCompositionStart={inputView.onCompositionStart}
				onDrop={inputView.onDrop}
				onInput={inputView.onInput}
				onKeyDown={inputView.onKeyDown}
				onKeyUp={inputView.onKeyUp}
				onPaste={inputView.onPaste}
				placeholder={view.messages.broadcastInputPlaceholder}
				ref={inputView.inputReference}
				spellCheck={false}
				type={state.hidden ? 'password' : 'text'}
			/>
			<Button onClick={inputView.toggleVisibility} size='sm' type='button' variant='outline'>
				{state.hidden ? view.messages.broadcastShowInput : view.messages.broadcastHideInput}
				<Kbd aria-hidden='true' className={styles['shortcut']}>
					Ctrl+Alt+Esc
				</Kbd>
			</Button>
			<Button onClick={inputView.end} size='sm' type='button'>
				{view.messages.broadcastEnd}
				<Kbd aria-hidden='true' className={styles['shortcut']}>
					Ctrl+Esc
				</Kbd>
			</Button>
		</div>
	);
}
