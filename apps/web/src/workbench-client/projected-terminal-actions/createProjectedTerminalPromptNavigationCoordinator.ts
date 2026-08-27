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

	return {
		dispose: () => {
			activeController?.abort();
			activeController = undefined;
		},
		run: async (signal, navigate) => {
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
