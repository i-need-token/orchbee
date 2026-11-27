import type { FileResult, GitPort } from '../../../file-session';

const UNAVAILABLE: Promise<FileResult<never>> = Promise.resolve({
	error: { code: 'unavailable' },
	kind: 'error',
});

/**
 * Source Control until the Git wire adapter carries it: every request is `unavailable`, and the
 * summary and relay streams stay silent.
 */
export const UNAVAILABLE_GIT_PORT: GitPort = {
	answer: () => UNAVAILABLE,
	checkIgnore: () => UNAVAILABLE,
	outcome: () => UNAVAILABLE,
	output: () => UNAVAILABLE,
	refs: () => UNAVAILABLE,
	run: () => UNAVAILABLE,
	show: () => UNAVAILABLE,
	statusPage: () => UNAVAILABLE,
	subscribe: () => () => undefined,
	subscribeRelay: () => () => undefined,
	userConfig: () => UNAVAILABLE,
};
