import { basename, extname } from 'node:path';
import { runtimeExports, startLoc } from '../utils.js';

const COMPONENT_NAME = /^[A-Z][A-Za-z0-9]*$/u;

/** @type {import('eslint').Rule.RuleModule} */
export const componentExportBoundary = {
	meta: {
		type: 'suggestion',
		docs: {
			description:
				'Keep one named component runtime export while allowing private helpers and types.',
		},
		schema: [],
	},
	create(context) {
		const filename = context.filename.split('\\').join('/');
		const name = basename(filename);
		if (
			!name.endsWith('.tsx') ||
			/\.(?:test|spec|stories)\.tsx$/u.test(name) ||
			!filename.includes('/src/') ||
			filename.includes('/__tests__/')
		)
			return {};

		return {
			'Program:exit'(program) {
				const stem = basename(filename, extname(filename));
				if (/^use[A-Z0-9]/u.test(stem)) return;
				const exports = runtimeExports(program);
				if (
					!COMPONENT_NAME.test(stem) &&
					!exports.some(
						(entry) => typeof entry.name === 'string' && COMPONENT_NAME.test(entry.name),
					)
				)
					return;
				const exportName = exports[0]?.name;
				if (exports.length === 1 && exportName === stem && COMPONENT_NAME.test(stem)) return;
				context.report({
					loc: startLoc(program),
					message:
						'Export exactly one named PascalCase component matching the filename; keep helpers private and use type exports for related contracts. See docs/frontend/react.md#tsx-file-boundary.',
				});
			},
		};
	},
};
