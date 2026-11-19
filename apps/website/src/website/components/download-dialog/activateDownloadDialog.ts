import type { ActivationContext } from '../../activation';
import { visitorFirst, visitorPlatform } from '../../release';

// The fragment that addresses the dialog. A page opened with it shows the dialog, and the open dialog
// writes it into the address, so the page can be shared that way.
const DOWNLOAD_HASH = '#download';

// What Tab and focus can reach inside the dialog.
const FOCUSABLE =
	'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Opens the download dialog from each download trigger (the navigation bar's and the home hero's) and when
// the page is addressed with #download. While it is open, Tab stays inside it; Escape, its close button or
// a click on the backdrop close it; focus returns to the trigger that opened it; and the page does not
// scroll. The platform group of the visitor's operating system is listed first, and its first installer is
// the primary one.
export function activateDownloadDialog({ document }: ActivationContext): void {
	const view = document.defaultView;
	const dialog = document.querySelector<HTMLElement>('[data-download-dialog]');
	if (view === null || dialog === null) return;
	const panel = dialog.querySelector<HTMLElement>('[data-download-panel]');
	if (panel === null) return;
	const list = panel.querySelector<HTMLElement>('[data-download-list]');
	const triggers = [...document.querySelectorAll<HTMLElement>('[data-download-trigger]')];
	let open = false;
	let opener: HTMLElement | null = null;
	let pressedBackdrop = false;

	const focusable = (): HTMLElement[] =>
		[...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
			(element) => element.closest('[hidden]') === null,
		);

	if (list !== null) {
		const groups = [...list.querySelectorAll<HTMLElement>('[data-download-group]')];
		const preferred = visitorPlatform(view.navigator.platform || view.navigator.userAgent);
		const ordered = visitorFirst(groups, (group) => group.dataset['downloadGroup'], preferred);
		for (const group of ordered) {
			list.append(group);
			for (const link of group.querySelectorAll<HTMLElement>('a')) {
				link.removeAttribute('data-primary');
			}
		}
		ordered[0]?.querySelector('a')?.setAttribute('data-primary', '');
	}

	const show = (trigger: HTMLElement | null): void => {
		if (open) return;
		open = true;
		opener = trigger;
		dialog.hidden = false;
		document.documentElement.setAttribute('data-download-open', '');
		if (view.location.hash !== DOWNLOAD_HASH) {
			view.history.replaceState(view.history.state, '', DOWNLOAD_HASH);
		}
		(list?.querySelector<HTMLElement>('a[href]') ?? focusable()[0])?.focus();
	};

	const hide = (): void => {
		if (!open) return;
		open = false;
		dialog.hidden = true;
		document.documentElement.removeAttribute('data-download-open');
		if (view.location.hash === DOWNLOAD_HASH) {
			view.history.replaceState(
				view.history.state,
				'',
				view.location.pathname + view.location.search,
			);
		}
		const target = opener ?? triggers[0];
		opener = null;
		target?.focus({ preventScroll: true });
	};

	for (const trigger of triggers) {
		trigger.addEventListener('click', (event) => {
			// Modified and other-button clicks keep the link's own behavior, such as a new tab.
			if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
				return;
			}
			event.preventDefault();
			show(trigger);
		});
	}

	dialog.addEventListener('pointerdown', (event) => {
		pressedBackdrop = event.target === dialog;
	});
	dialog.addEventListener('click', (event) => {
		const { target } = event;
		if (target instanceof Element && target.closest('[data-download-close]') !== null) {
			hide();
		} else if (pressedBackdrop && target === dialog) {
			hide();
		}
		pressedBackdrop = false;
	});

	document.addEventListener('keydown', (event) => {
		if (!open) return;
		if (event.key === 'Escape') {
			event.preventDefault();
			hide();
			return;
		}
		if (event.key !== 'Tab') return;
		const items = focusable();
		const first = items[0];
		const last = items[items.length - 1];
		if (first === undefined || last === undefined) {
			event.preventDefault();
			return;
		}
		const active = document.activeElement;
		const inside = active !== null && panel.contains(active);
		if (event.shiftKey) {
			if (!inside || active === first) {
				event.preventDefault();
				last.focus();
			}
		} else if (!inside || active === last) {
			event.preventDefault();
			first.focus();
		}
	});

	document.addEventListener('focusin', (event) => {
		if (!open || (event.target instanceof Node && panel.contains(event.target))) return;
		focusable()[0]?.focus();
	});

	view.addEventListener('hashchange', () => {
		if (view.location.hash === DOWNLOAD_HASH) show(null);
		else hide();
	});
	if (view.location.hash === DOWNLOAD_HASH) show(null);
}
