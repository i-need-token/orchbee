// @vitest-environment jsdom

import { ActionRuntime, defineAction, noActionInput } from '@orchbee/actions';
import { OrchbeeI18nProvider, useOrchbeeIntl } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, useLayoutEffect, type ReactNode } from 'react';
import { afterEach, describe, expect, it } from 'vitest';

import { useWorkbenchSearchCatalog } from '../useWorkbenchSearchCatalog';

const dynamicAction = defineAction({
	category: {
		id: 'navigation',
		title: { defaultMessage: 'Navigation', id: 'test.navigation' },
	},
	execution: {
		concurrency: 'parallel',
		concurrencyKey: () => 'dynamic',
		mode: 'foreground',
	},
	getAvailability: () => ({ kind: 'enabled' }),
	handler: { execute: () => ({ kind: 'completed' }) },
	id: 'dynamic.action',
	input: noActionInput,
	title: { defaultMessage: 'Dynamic action', id: 'test.dynamic.action' },
});

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useWorkbenchSearchCatalog', () => {
	it('catches registry changes between rendering and subscribing', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		function RegisterDuringCommit({ children }: { readonly children: ReactNode }) {
			useLayoutEffect(() => {
				const registration = runtime.register(scope, dynamicAction);
				return () => registration.dispose();
			}, []);
			return createElement(I18nWrapper, null, children);
		}
		const rendered = renderHook(
			() =>
				useWorkbenchSearchCatalog({
					intl: useOrchbeeIntl(),
					runtime,
					scope,
					shortcutLabelForAction: () => undefined,
				}),
			{ wrapper: RegisterDuringCommit },
		);

		expect(rendered.result.current.actionResults.map((result) => result.actionId)).toEqual([
			'dynamic.action',
		]);
	});

	it('reprojects the local catalog when the Action registry changes', () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const rendered = renderHook(
			() => {
				const intl = useOrchbeeIntl();
				return useWorkbenchSearchCatalog({
					intl,
					runtime,
					scope,
					shortcutLabelForAction: () => undefined,
				});
			},
			{ wrapper: I18nWrapper },
		);

		expect(rendered.result.current.actionResults).toEqual([]);
		act(() => {
			runtime.register(scope, dynamicAction);
		});

		expect(rendered.result.current.actionResults.map((result) => result.actionId)).toContain(
			'dynamic.action',
		);
	});
});
