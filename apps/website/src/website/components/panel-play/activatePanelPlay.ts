import type { ActivationContext } from '../../activation';
import { panelCommand, type PanelPlayClient } from './panel-play-clients';
import {
	closePanel,
	dragDivider,
	equalizePanels,
	focusPanel,
	initialPanelLayout,
	moveFocus,
	resizePanel,
	splitPanel,
	toggleZoom,
	type PanelAction,
	type PanelDirection,
	type PanelOutcome,
} from '../panel-tree/panel-tree';
import { createPanelView } from '../panel-tree/createPanelView';

function isClient(value: string | undefined): value is PanelPlayClient {
	return value === 'terminal' || value === 'desktop';
}

function isAction(value: string | undefined): value is PanelAction {
	return (
		value === 'right' ||
		value === 'down' ||
		value === 'left' ||
		value === 'up' ||
		value === 'zoom' ||
		value === 'equal' ||
		value === 'close'
	);
}

// Notices the page carries, each a prerendered element under its key; the hint shows a copy.
function readNotices(root: Element): Map<string, Element> {
	const notices = new Map<string, Element>();
	const template = root.querySelector('template[data-panel-strings]');
	if (!(template instanceof HTMLTemplateElement)) return notices;
	for (const notice of template.content.querySelectorAll('[data-key]'))
		notices.set(notice.getAttribute('data-key') ?? '', notice);
	return notices;
}

function play(root: HTMLElement, client: PanelPlayClient): void {
	const host = root.querySelector<HTMLElement>('[data-panel-host]');
	const window = host?.querySelector('[data-window-fit]')?.firstElementChild;
	const hint = root.querySelector<HTMLElement>('[data-panel-hint]');
	const chips = root.querySelector<HTMLElement>('[data-panel-chips]');
	if (!(window instanceof HTMLElement) || host === null || hint === null || chips === null) return;
	const view = createPanelView(window);
	if (view === undefined) return;
	const notices = readNotices(root);

	// The page was drawn with the starting layout; the model starts from the same one.
	let layout = initialPanelLayout();

	const say = (key: string): void => {
		const notice = notices.get(key);
		hint.replaceChildren(...(notice === undefined ? [] : [...notice.cloneNode(true).childNodes]));
	};
	const show = (outcome: PanelOutcome): void => {
		switch (outcome.kind) {
			case 'layout':
				layout = outcome.layout;
				view.draw(layout, outcome.fresh);
				say(`act:${outcome.done}`);
				return;
			case 'ratio':
				layout = outcome.layout;
				view.showRatio(layout, outcome.split);
				return;
			case 'focus':
				layout = outcome.layout;
				view.showFocus(layout);
				return;
			case 'refused':
				say(outcome.reason);
				return;
			case 'unchanged':
				return;
		}
	};
	const split = (where: PanelDirection): void => {
		// A maximized Panel is restored before it is measured for the split.
		if (layout.zoomed) {
			layout = { ...layout, zoomed: false };
			view.draw(layout);
		}
		show(splitPanel(layout, where, view.room(layout.focus)));
	};
	const resize = (where: PanelDirection): void => {
		const outcome = resizePanel(layout, where);
		show(outcome);
		if (outcome.kind === 'ratio') say('act:resize');
	};
	const run = (act: PanelAction): void => {
		if (act === 'zoom') show(toggleZoom(layout));
		else if (act === 'equal') show(equalizePanels(layout));
		else if (act === 'close') show(closePanel(layout, layout.focus));
		else split(act);
	};

	host.tabIndex = 0;
	host.setAttribute('role', 'application');
	const label = notices.get('label')?.textContent;
	if (label !== undefined && label !== null) host.setAttribute('aria-label', label);
	chips.hidden = false;
	hint.hidden = false;
	say('idle');

	for (const chip of chips.querySelectorAll<HTMLButtonElement>('[data-panel-act]')) {
		const act = chip.dataset['panelAct'];
		if (isAction(act))
			chip.addEventListener('click', () => {
				run(act);
			});
	}

	host.addEventListener('click', (event) => {
		const target = event.target instanceof Element ? event.target : null;
		const leaf = target?.closest<HTMLElement>('[data-leaf]');
		if (leaf === null || leaf === undefined) return;
		const id = Number(leaf.dataset['leaf']);
		show(target?.closest('[data-panel-close]') ? closePanel(layout, id) : focusPanel(layout, id));
	});

	host.addEventListener('pointerdown', (event) => {
		const divider = event.target instanceof Element ? event.target.closest('[data-divider]') : null;
		const element = divider?.parentElement;
		if (!(divider instanceof HTMLElement) || element === null || element === undefined) return;
		event.preventDefault();
		host.focus({ preventScroll: true });
		const key = Number(element.getAttribute('data-split'));
		const across = element.getAttribute('data-axis') === 'row';
		divider.setPointerCapture(event.pointerId);
		const drag = (move: PointerEvent): void => {
			const box = element.getBoundingClientRect();
			const ratio = across
				? (move.clientX - box.left) / box.width
				: (move.clientY - box.top) / box.height;
			show(dragDivider(layout, key, ratio));
		};
		const end = (): void => {
			divider.removeEventListener('pointermove', drag);
			divider.removeEventListener('pointerup', end);
			divider.removeEventListener('pointercancel', end);
		};
		divider.addEventListener('pointermove', drag);
		divider.addEventListener('pointerup', end);
		divider.addEventListener('pointercancel', end);
	});

	host.addEventListener('keydown', (event) => {
		const command = panelCommand(client, event);
		if (command === undefined) return;
		event.preventDefault();
		if (command.kind === 'leave') host.blur();
		else if (command.kind === 'prefix') say('prefix');
		else if (command.kind === 'act') run(command.act);
		else if (command.kind === 'resize') resize(command.where);
		else show(moveFocus(layout, command.where, view.boxes()));
	});
}

// Lets the visitor work the Panels of each try-it window: keys while the window has focus, the
// action buttons, a click on a Panel and a drag on a divider. It works under reduced motion too,
// because nothing in it moves by itself.
export function activatePanelPlay({ document }: ActivationContext): void {
	for (const root of document.querySelectorAll<HTMLElement>('[data-panel-play]')) {
		const client = root.dataset['panelPlay'];
		if (isClient(client)) play(root, client);
	}
}
