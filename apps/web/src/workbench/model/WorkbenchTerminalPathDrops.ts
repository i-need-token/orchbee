import type {
	ActionExecution,
	ActionInvocationSource,
	ActionRuntime,
	ActionScope,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { FILE_DRAG } from '../../file-drag';
import {
	registerPointerDropTarget,
	type PointerDrag,
	type PointerDropTarget,
} from '../../pointer-drag';
import {
	TERMINAL_ACTION_IDS,
	type WorkbenchTerminalPathsInput,
} from '../actions/terminal-action-contracts';
import type { WorkbenchAuthorityNotice } from './workbench-authority-notice';

/** Runs an Action a person invoked, and says its refusal or failure as the Workbench's notice does. */
type ActionOutcomeReport = (
	source: ActionInvocationSource,
	start: () => ActionExecution,
	named: WorkbenchAuthorityNotice['descriptor'],
) => unknown;

/** What `over` last answered, for the drag and the terminal it answered for. */
interface DropAnswer {
	readonly accepted: boolean;
	readonly drag: PointerDrag;
	readonly scope: ActionScope;
}

/**
 * Drops of the File Bar's files and folders on terminals. A drop types their paths into a terminal
 * through its `terminal.paths.insert`, as a drop of files from the system types theirs, formatted for
 * the shell of the machine that holds them. Terminal viewports register while they show, so a drop
 * that goes to the current terminal, such as one on the overlay's scrim, finds that terminal too.
 */
export class WorkbenchTerminalPathDrops {
	readonly #runtime: ActionRuntime;
	readonly #report: ActionOutcomeReport;
	readonly #scopes = new Map<string, ActionScope>();
	#answer: DropAnswer | null = null;

	constructor(runtime: ActionRuntime, report: ActionOutcomeReport) {
		this.#runtime = runtime;
		this.#report = report;
	}

	/**
	 * Takes drops on `viewport` for the terminal it shows, `presentationId`, whose Actions resolve in
	 * `scope`, until the returned function runs. `showDropTarget` says whether a drop there would land.
	 */
	registerTerminal(
		presentationId: string,
		scope: ActionScope,
		viewport: Element,
		showDropTarget: (shown: boolean) => void,
	): () => void {
		this.#scopes.set(presentationId, scope);
		const unregisterTarget = registerPointerDropTarget(
			viewport,
			this.#target(
				() => scope,
				showDropTarget,
				() => undefined,
			),
		);
		return () => {
			unregisterTarget();
			if (this.#scopes.get(presentationId) === scope) this.#scopes.delete(presentationId);
		};
	}

	/**
	 * A target whose drops go to the terminal `currentPresentationId` names, of those registered.
	 * `beforeDrop` runs as a drop lands, before the paths are typed; the target shows nothing itself.
	 */
	currentTerminalTarget(
		currentPresentationId: () => string | null,
		beforeDrop: () => void,
	): PointerDropTarget {
		return this.#target(
			() => {
				const presentationId = currentPresentationId();
				return presentationId === null ? null : (this.#scopes.get(presentationId) ?? null);
			},
			() => undefined,
			beforeDrop,
		);
	}

	#target(
		resolveScope: () => ActionScope | null,
		showDropTarget: (shown: boolean) => void,
		beforeDrop: () => void,
	): PointerDropTarget {
		return {
			drop: (drag) => {
				this.#answer = null;
				showDropTarget(false);
				const scope = resolveScope();
				const input = pathsInput(drag);
				if (scope === null || input === undefined) return;
				beforeDrop();
				this.#report(
					'pointer',
					() =>
						this.#runtime.execute(TERMINAL_ACTION_IDS.insertPaths, input, {
							scope,
							source: 'pointer',
						}),
					workbenchMessages.terminalPathsNotInserted,
				);
			},
			leave: () => {
				this.#answer = null;
				showDropTarget(false);
			},
			over: (drag) => {
				const accepted = this.#accepts(drag, resolveScope());
				showDropTarget(accepted);
				return accepted;
			},
		};
	}

	/**
	 * Whether the terminal of `scope` takes the drag's paths now. `over` asks on every frame, so the
	 * answer holds for the drag's stay on one terminal.
	 */
	#accepts(drag: PointerDrag, scope: ActionScope | null): boolean {
		if (scope === null) return false;
		const answer = this.#answer;
		if (answer?.drag === drag && answer.scope === scope) return answer.accepted;
		const input = pathsInput(drag);
		const accepted =
			input !== undefined &&
			this.#runtime.resolveAction(TERMINAL_ACTION_IDS.insertPaths, input, {
				scope,
				source: 'pointer',
			})?.availability.kind === 'enabled';
		this.#answer = { accepted, drag, scope };
		return accepted;
	}
}

/** The paths a drag of the File Bar's files and folders carries, for its machine's shell. */
function pathsInput(drag: PointerDrag): WorkbenchTerminalPathsInput | undefined {
	const files = FILE_DRAG.read(drag);
	if (files === undefined) return undefined;
	return { paths: files.paths, platform: files.rules.os === 'windows' ? 'windows' : 'posix' };
}
