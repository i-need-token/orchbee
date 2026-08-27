type ProjectedTerminalTextViewerPreparationResult<Result> =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'prepared'; readonly value: Result };

export interface ProjectedTerminalTextViewerPreparationCoordinator {
	dispose(): void;
	run<Result>(
		signal: AbortSignal,
		prepare: (signal: AbortSignal) => Promise<Result>,
	): Promise<ProjectedTerminalTextViewerPreparationResult<Result>>;
}

export function createProjectedTerminalTextViewerPreparationCoordinator(): ProjectedTerminalTextViewerPreparationCoordinator {
	let activeController: AbortController | undefined;

	return {
		dispose: () => {
			activeController?.abort();
			activeController = undefined;
		},
		run: async (signal, prepare) => {
			const controller = new AbortController();
			activeController?.abort();
			activeController = controller;
			const abort = () => controller.abort();
			if (signal.aborted) {
				controller.abort();
			} else {
				signal.addEventListener('abort', abort, { once: true });
			}

			try {
				if (controller.signal.aborted) return { kind: 'cancelled' };
				const value = await prepare(controller.signal);
				if (controller.signal.aborted || signal.aborted) return { kind: 'cancelled' };
				return { kind: 'prepared', value };
			} finally {
				signal.removeEventListener('abort', abort);
				if (activeController === controller) activeController = undefined;
			}
		},
	};
}
