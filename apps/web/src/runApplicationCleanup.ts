/** Release every retired application resource before preserving its cleanup failures. */
export function runApplicationCleanup(
	steps: readonly (() => void)[],
	failureMessage: string,
): void {
	const failures: unknown[] = [];
	for (const step of steps) {
		try {
			step();
		} catch (error) {
			failures.push(error);
		}
	}
	if (failures.length === 1) throw failures[0];
	if (failures.length > 1) throw new AggregateError(failures, failureMessage);
}
