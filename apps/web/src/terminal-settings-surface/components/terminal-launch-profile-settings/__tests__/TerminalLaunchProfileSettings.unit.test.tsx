// @vitest-environment jsdom

import {
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileIdSchema,
	type DesktopTerminalLaunchProfile,
	type DesktopTerminalLaunchProfileCatalog,
	type DesktopTerminalLaunchProfileId,
} from '@orchbee/desktop-client-protocol';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { getByLabelText } from '@testing-library/react';
import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	DesktopTerminalLaunchProfileClientOperationResult,
	DesktopTerminalLaunchProfileClientSnapshot,
	DesktopTerminalLaunchProfileControl,
} from '../../../../terminal-launch-profile-control';
import { TerminalLaunchProfileSettings } from '../TerminalLaunchProfileSettings';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

describe('TerminalLaunchProfileSettings', () => {
	it('edits a literal custom launch and explains the future-terminal boundary', async () => {
		const harness = await createHarness();

		expect(harness.container.textContent).toContain(
			'The default profile is captured when a new terminal starts.',
		);
		expect(
			requiredElement<HTMLButtonElement>(
				harness.container.querySelector('[data-slot="terminal-launch-profile-option"]'),
			).getAttribute('aria-pressed'),
		).toBe('true');
		expect(harness.container.querySelector('[role="listbox"]')).toBeNull();
		expect(
			requiredElement<HTMLInputElement>(harness.container.querySelector('#terminal-profile-name'))
				.required,
		).toBe(false);
		const command = requiredElement<HTMLSelectElement>(
			harness.container.querySelector('#terminal-profile-command'),
		);
		await act(async () => {
			command.value = 'custom-executable';
			command.dispatchEvent(new Event('change', { bubbles: true }));
		});
		const executable = requiredElement<HTMLInputElement>(
			harness.container.querySelector('#terminal-profile-executable'),
		);
		await act(async () => {
			setInputValue(executable, '/usr/local/bin/fish');
			executable.dispatchEvent(new Event('input', { bubbles: true }));
		});
		const addArgument = buttonWithText(harness.container, 'Add argument');
		await act(async () => {
			addArgument.click();
			addArgument.click();
		});
		const firstArgument = getByLabelText(harness.container, 'Argument 1');
		const secondArgument = getByLabelText(harness.container, 'Argument 2');
		if (
			!(firstArgument instanceof HTMLInputElement) ||
			!(secondArgument instanceof HTMLInputElement)
		) {
			throw new Error('Expected terminal argument inputs.');
		}
		await act(async () => {
			setInputValue(firstArgument, '--name');
			firstArgument.dispatchEvent(new Event('input', { bubbles: true }));
			setInputValue(secondArgument, 'two words');
			secondArgument.dispatchEvent(new Event('input', { bubbles: true }));
		});
		await act(async () => buttonWithText(harness.container, 'Save changes').click());
		await vi.waitFor(() =>
			expect(harness.commands.at(-1)).toMatchObject({
				profile: {
					command: {
						arguments: ['--name', 'two words'],
						executable: '/usr/local/bin/fish',
						kind: 'custom-executable',
					},
				},
				type: 'desktop-terminal-launch-profile-update-requested',
			}),
		);

		await harness.dispose();
	});

	it('preserves the edited draft when Main cannot commit it', async () => {
		const harness = await createHarness();
		const command = requiredElement<HTMLSelectElement>(
			harness.container.querySelector('#terminal-profile-command'),
		);
		await act(async () => {
			command.value = 'custom-executable';
			command.dispatchEvent(new Event('change', { bubbles: true }));
		});
		const executable = requiredElement<HTMLInputElement>(
			harness.container.querySelector('#terminal-profile-executable'),
		);
		await act(async () => {
			setInputValue(executable, '/usr/local/bin/preserved-draft');
			executable.dispatchEvent(new Event('input', { bubbles: true }));
		});
		harness.client.failNextUpdate();

		await act(async () => buttonWithText(harness.container, 'Save changes').click());
		await vi.waitFor(() =>
			expect(harness.container.querySelector('[role="alert"]')?.textContent).toContain(
				'could not be saved',
			),
		);

		expect(
			requiredElement<HTMLInputElement>(
				harness.container.querySelector('#terminal-profile-executable'),
			).value,
		).toBe('/usr/local/bin/preserved-draft');
		await harness.dispose();
	});

	it('creates, selects, and explicitly confirms a reset', async () => {
		const harness = await createHarness();
		const name = requiredElement<HTMLInputElement>(
			harness.container.querySelector("input[placeholder='Profile name']"),
		);
		await act(async () => {
			setInputValue(name, 'Development');
			name.dispatchEvent(new Event('input', { bubbles: true }));
		});
		await act(async () => buttonWithText(harness.container, 'Add profile').click());
		await vi.waitFor(() => expect(harness.container.textContent).toContain('Development'));
		await act(async () => buttonWithText(harness.container, 'Make default').click());
		await vi.waitFor(() => {
			expect(harness.commands.at(-1)?.type).toBe(
				'desktop-terminal-launch-profile-default-requested',
			);
		});
		expect(buttonWithText(harness.container, 'Delete').disabled).toBe(true);

		await act(async () => buttonWithText(harness.container, 'Reset profiles').click());
		expect(harness.container.textContent).toContain('Reset all profiles');
		await act(async () => buttonWithText(harness.container, 'Reset all profiles').click());
		await vi.waitFor(() => expect(harness.container.textContent).not.toContain('Development'));
		expect(harness.commands.at(-1)?.type).toBe('desktop-terminal-launch-profile-reset-requested');

		await harness.dispose();
	});
});

async function createHarness() {
	const client = new TestTerminalLaunchProfileControl();
	const container = document.createElement('div');
	document.body.append(container);
	const root = createRoot(container);
	await act(async () => {
		root.render(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(TerminalLaunchProfileSettings, { client }),
			),
		);
	});
	return {
		client,
		commands: client.commands,
		container,
		dispose: async (): Promise<void> => {
			await act(async () => root.unmount());
			container.remove();
		},
	};
}

function builtInCatalog(): DesktopTerminalLaunchProfileCatalog {
	return DesktopTerminalLaunchProfileCatalogSchema.parse({
		defaultProfileId: 'system-default',
		profiles: [
			{
				command: { kind: 'system-shell', login: false },
				id: 'system-default',
				name: null,
			},
		],
		version: 1,
	});
}

interface RecordedCommand {
	readonly profile?: DesktopTerminalLaunchProfile | undefined;
	readonly type:
		| 'desktop-terminal-launch-profile-create-requested'
		| 'desktop-terminal-launch-profile-default-requested'
		| 'desktop-terminal-launch-profile-delete-requested'
		| 'desktop-terminal-launch-profile-reset-requested'
		| 'desktop-terminal-launch-profile-update-requested';
}

class TestTerminalLaunchProfileControl implements DesktopTerminalLaunchProfileControl {
	readonly commands: RecordedCommand[] = [];
	readonly #listeners = new Set<() => void>();
	#updateMustFail = false;
	#snapshot: DesktopTerminalLaunchProfileClientSnapshot = {
		capabilities: { loginShell: true },
		catalog: builtInCatalog(),
		failureCode: null,
		pending: false,
		persistenceIssue: null,
		source: 'stored',
	};

	createProfile = async (
		name: string,
		sourceProfileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> => {
		this.commands.push({ type: 'desktop-terminal-launch-profile-create-requested' });
		const sourceIndex = this.#snapshot.catalog.profiles.findIndex(
			(profile) => profile.id === sourceProfileId,
		);
		const source = this.#snapshot.catalog.profiles[sourceIndex];
		if (source === undefined) return { kind: 'unavailable' };
		const profiles = [...this.#snapshot.catalog.profiles];
		profiles.splice(sourceIndex + 1, 0, {
			...source,
			id: DesktopTerminalLaunchProfileIdSchema.parse('profile-created'),
			name,
		});
		return this.#commit({ ...this.#snapshot.catalog, profiles });
	};

	deleteProfile = async (
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> => {
		this.commands.push({ type: 'desktop-terminal-launch-profile-delete-requested' });
		return this.#commit({
			...this.#snapshot.catalog,
			profiles: this.#snapshot.catalog.profiles.filter((profile) => profile.id !== profileId),
		});
	};

	getSnapshot = (): DesktopTerminalLaunchProfileClientSnapshot => this.#snapshot;

	failNextUpdate(): void {
		this.#updateMustFail = true;
	}

	reset = async (): Promise<DesktopTerminalLaunchProfileClientOperationResult> => {
		this.commands.push({ type: 'desktop-terminal-launch-profile-reset-requested' });
		return this.#commit(builtInCatalog());
	};

	selectDefault = async (
		profileId: DesktopTerminalLaunchProfileId,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> => {
		this.commands.push({ type: 'desktop-terminal-launch-profile-default-requested' });
		return this.#commit({ ...this.#snapshot.catalog, defaultProfileId: profileId });
	};

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	updateProfile = async (
		profile: DesktopTerminalLaunchProfile,
	): Promise<DesktopTerminalLaunchProfileClientOperationResult> => {
		this.commands.push({ profile, type: 'desktop-terminal-launch-profile-update-requested' });
		if (this.#updateMustFail) {
			this.#updateMustFail = false;
			this.#snapshot = {
				...this.#snapshot,
				catalog: DesktopTerminalLaunchProfileCatalogSchema.parse(
					JSON.parse(JSON.stringify(this.#snapshot.catalog)),
				),
				failureCode: 'write-failed',
				persistenceIssue: 'write-failed',
			};
			for (const listener of this.#listeners) listener();
			return { code: 'write-failed', kind: 'failed', snapshot: this.#snapshot };
		}
		return this.#commit({
			...this.#snapshot.catalog,
			profiles: this.#snapshot.catalog.profiles.map((candidate) =>
				candidate.id === profile.id ? profile : candidate,
			),
		});
	};

	#commit(
		catalog: DesktopTerminalLaunchProfileCatalog,
	): DesktopTerminalLaunchProfileClientOperationResult {
		this.#snapshot = { ...this.#snapshot, catalog };
		for (const listener of this.#listeners) listener();
		return { kind: 'completed', snapshot: this.#snapshot };
	}
}

function buttonWithText(container: ParentNode, text: string): HTMLButtonElement {
	return requiredElement(
		[...container.querySelectorAll<HTMLButtonElement>('button')].find(
			(button) => button.textContent?.trim() === text,
		) ?? null,
	);
}

function requiredElement<ElementType extends Element>(element: ElementType | null): ElementType {
	if (element === null) throw new Error('Required test element is missing.');
	return element;
}

function setInputValue(input: HTMLInputElement, value: string): void {
	const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set;
	setter?.call(input, value);
}
