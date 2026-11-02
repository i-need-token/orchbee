export interface ServerHostColors {
	readonly background: number;
	readonly foreground: number;
}

/**
 * The colors a window paints around its terminals, offered as a TUI offers its outer terminal's. It
 * has none while the Session's preferences give terminals colors of their own.
 */
export interface ServerHostColorsSource {
	current(): ServerHostColors | null;
	subscribe(listener: () => void): () => void;
}

interface ServerHostColorsReportOptions {
	/** Queues a step behind the terminal's input; false when nothing more is accepted. */
	readonly enqueue: (step: () => Promise<void>) => boolean;
	readonly source: ServerHostColorsSource;
	/** Reports the colors under the current controller; true once an answer settled them. */
	readonly submit: (colors: ServerHostColors) => Promise<boolean>;
}

function hostColorsKey(colors: ServerHostColors): string {
	return `${colors.foreground}:${colors.background}`;
}

/**
 * Reports a window's colors to the terminal it controls, as the TUI reports its outer terminal's:
 * after each grant of control and whenever they change. The Worker recolors only a terminal that
 * follows its controller, and remembers the colors for the next one launched. A refusal counts as
 * reported so nothing repeats it, and a lost answer waits for the next change or grant.
 */
export class ServerHostColorsReport {
	readonly #options: ServerHostColorsReportOptions;
	readonly #unsubscribe: () => void;
	/** The colors the last queued step will find unless they change again. */
	#queued: string | null = null;
	/** The colors an answer last settled. Steps run in order, so each compares with its predecessors. */
	#reported: string | null = null;

	constructor(options: ServerHostColorsReportOptions) {
		this.#options = options;
		this.#unsubscribe = options.source.subscribe(() => this.#request());
	}

	/** Another Client may have reported its own colors while this one lacked control. */
	granted(): void {
		this.#queued = null;
		this.#reported = null;
		this.#request();
	}

	dispose(): void {
		this.#unsubscribe();
	}

	#request(): void {
		const colors = this.#options.source.current();
		if (colors === null) return;
		const key = hostColorsKey(colors);
		// Comparing with the settled colors instead would miss a return to them during a report.
		if (key === this.#queued) return;
		const queued = this.#options.enqueue(async () => {
			const latest = this.#options.source.current();
			if (latest === null) return;
			const latestKey = hostColorsKey(latest);
			if (latestKey === this.#reported) return;
			if (await this.#options.submit(latest)) this.#reported = latestKey;
		});
		if (queued) this.#queued = key;
	}
}
