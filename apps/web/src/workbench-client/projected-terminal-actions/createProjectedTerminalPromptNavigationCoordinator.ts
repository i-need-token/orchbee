type ProjectedTerminalPromptNavigationResult = 'cancelled' | 'moved' | 'unchanged';

export interface ProjectedTerminalPromptNavigationCoordinator {
	dispose(): void;
	run(
		signal: AbortSignal,
		navigate: (signal: AbortSignal) => Promise<boolean>,
	): Promise<ProjectedTerminalPromptNavigationResult>;
}

export function createProjectedTerminalPromptNavigationCoordinator(): ProjectedTerminalPromptNavigationCoordinator {
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
		run: async (signal, navigate) => {
			if (disposed || signal.aborted) return 'cancelled';
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
				if (controller.signal.aborted) return 'cancelled';
				const moved = await navigate(controller.signal);
				if (controller.signal.aborted || signal.aborted) return 'cancelled';
				return moved ? 'moved' : 'unchanged';
			} finally {
				signal.removeEventListener('abort', abort);
				if (activeController === controller) activeController = undefined;
			}
		},
	};
}
