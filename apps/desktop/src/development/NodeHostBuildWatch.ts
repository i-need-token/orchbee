import type { FSWatcher } from 'node:fs';

import type {
	DesktopDevelopmentHostBuildWatch,
	DesktopDevelopmentSupervisedProcess,
} from './DesktopDevelopmentRuntime';

export class NodeHostBuildWatch implements DesktopDevelopmentHostBuildWatch {
	readonly #buildProcess: Pick<DesktopDevelopmentSupervisedProcess, 'terminate'>;
	readonly #fileWatcher: Pick<FSWatcher, 'close'>;
	readonly #clearDebounce: () => void;
	readonly #drain: () => Promise<void>;

	constructor(
		buildProcess: Pick<DesktopDevelopmentSupervisedProcess, 'terminate'>,
		fileWatcher: Pick<FSWatcher, 'close'>,
		clearDebounce: () => void,
		drain: () => Promise<void>,
	) {
		this.#buildProcess = buildProcess;
		this.#fileWatcher = fileWatcher;
		this.#clearDebounce = clearDebounce;
		this.#drain = drain;
	}

	async close(): Promise<void> {
		this.#clearDebounce();
		const failures: unknown[] = [];
		try {
			this.#fileWatcher.close();
		} catch (error: unknown) {
			failures.push(error);
		}
		try {
			await this.#buildProcess.terminate();
		} catch (error: unknown) {
			failures.push(error);
		}
		try {
			await this.#drain();
		} catch (error: unknown) {
			failures.push(error);
		}
		if (failures.length > 0) throw failures[0];
	}
}
