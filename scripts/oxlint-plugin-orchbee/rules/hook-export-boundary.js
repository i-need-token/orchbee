import { basename, extname } from 'node:path';
import { runtimeExports, startLoc } from '../utils.js';

const HOOK_NAME = /^use[A-Z0-9][A-Za-z0-9]*$/u;

/** @type {import('eslint').Rule.RuleModule} */
export const hookExportBoundary = {
	meta: {
		type: 'suggestion',
		docs: {
			description:
				'Keep one named Hook runtime export while allowing private helpers and related types.',
		},
		schema: [],
	},
	create(context) {
		const filename = context.filename.split('\\').join('/');
		const name = basename(filename);
		if (
			!/\.tsx?$/u.test(name) ||
			name === 'index.ts' ||
			/\.(?:test|spec|stories)\.tsx?$/u.test(name) ||
			!filename.includes('/src/') ||
			filename.includes('/__tests__/') ||
			filename.includes('/contexts/')
		)
			return {};

		return {
			'Program:exit'(program) {
				const stem = basename(filename, extname(filename));
				const exports = runtimeExports(program);
				if (
					!/^use(?:-|[A-Z0-9])/u.test(stem) &&
					!exports.some((entry) => typeof entry.name === 'string' && HOOK_NAME.test(entry.name))
				)
					return;
				if (exports.length === 1 && exports[0]?.name === stem && HOOK_NAME.test(stem)) return;
				context.report({
					loc: startLoc(program),
					message:
						'Export exactly one named Hook matching the filename; keep helpers private and allow related type exports beside the Hook. See docs/frontend/react.md#hook-file-boundary.',
				});
			},
		};
	},
};
