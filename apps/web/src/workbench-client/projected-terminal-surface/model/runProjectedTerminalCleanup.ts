/** Release every retired terminal resource before preserving its cleanup failures. */
export function runProjectedTerminalCleanup(steps: readonly (() => void)[]): void {
	const failures: unknown[] = [];
	for (const step of steps) {
		try {
			step();
		} catch (error) {
			failures.push(error);
		}
	}
	if (failures.length === 1) throw failures[0];
	if (failures.length > 1) throw new AggregateError(failures, 'Projected terminal cleanup failed');
}
