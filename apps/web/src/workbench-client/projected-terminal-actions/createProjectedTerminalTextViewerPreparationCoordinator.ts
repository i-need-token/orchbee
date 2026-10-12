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
	let disposed = false;

	return {
		dispose: () => {
			if (disposed) return;
			disposed = true;
			const controller = activeController;
			activeController = undefined;
			controller?.abort();
		},
		run: async (signal, prepare) => {
			if (disposed || signal.aborted) return { kind: 'cancelled' };
			const controller = new AbortController();
			const previous = activeController;
			// Abort listeners can synchronously replace this request or dispose its owner.
			activeController = controller;
			previous?.abort();
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
