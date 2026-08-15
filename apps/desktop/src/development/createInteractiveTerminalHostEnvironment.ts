/** @internal Development-only environment for the Host that launches interactive shells. */
export function createInteractiveTerminalHostEnvironment(
	environment: NodeJS.ProcessEnv,
): NodeJS.ProcessEnv {
	const interactiveEnvironment: NodeJS.ProcessEnv = {};
	for (const [name, value] of Object.entries(environment)) {
		if (name === 'GIT_PAGER' || name === 'PAGER') continue;
		interactiveEnvironment[name] = value;
	}
	return interactiveEnvironment;
}
