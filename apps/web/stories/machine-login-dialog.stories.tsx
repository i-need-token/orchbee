import type { Meta, StoryObj } from 'storybook-react-rsbuild';

import {
	MachineLoginDialog,
	type TerminalSettingsMachines,
	type TerminalSettingsMachinesView,
} from '../src/terminal-settings-surface';

type Login = NonNullable<TerminalSettingsMachinesView['login']>;

const ENGLISH_LABELS: Login['labels'] = {
	cancel: 'Cancel',
	close: 'Close',
	privacy: 'Goes only to ssh. Orchbee does not keep it.',
	rememberPrivacy: 'Goes to ssh, then to the system password store once logged in.',
	sending: 'Logging in…',
	submit: 'Log in',
	title: 'Log in to machine',
	typeYes: 'Type yes to trust this host.',
};

const CHINESE_LABELS: Login['labels'] = {
	cancel: '取消',
	close: '关闭',
	privacy: null,
	rememberPrivacy: '交给 ssh，登录成功后存入系统的密码存放处。',
	sending: '正在登录…',
	submit: '信任并继续',
	title: '登录机器',
	typeYes: '输入 yes 以信任这台主机。',
};

/** The machines process with one login question and nothing else. */
function source(login: Login): TerminalSettingsMachines {
	const view: TerminalSettingsMachinesView = {
		add: {
			form: null,
			labels: {
				add: 'Add',
				addMachine: 'Add machine…',
				address: 'Address',
				cancel: 'Cancel',
				color: 'Color',
				hosts: 'Hosts in SSH config',
				name: 'Name (optional)',
				nameHint: 'Leave blank to use the host name.',
				title: 'Add machine',
			},
		},
		added: null,
		catalog: 'read',
		hub: null,
		local: { long: '', parts: [], summary: '' },
		login,
		machines: [],
		notice: null,
		passwords: true,
	};
	return {
		getSnapshot: () => view,
		request: () => undefined,
		showCapacity: () => () => undefined,
		subscribe: () => () => undefined,
	};
}

const password: Login = {
	asked: ['Password for dev@10.0.0.8:'],
	ended: null,
	kind: 'password',
	labels: ENGLISH_LABELS,
	login: 1,
	machine: 'devbox · dev@10.0.0.8',
	notice: null,
	question: 1,
	remember: { label: 'Remember password', ticked: false },
	rememberLine: null,
	said: ['ssh: dev@10.0.0.8: Permission denied (publickey).'],
	sending: false,
};

const meta = {
	title: 'Machine Login',
	component: MachineLoginDialog,
	args: { machines: source(password) },
} satisfies Meta<typeof MachineLoginDialog>;

export default meta;

type Story = StoryObj<typeof meta>;

/** The person's own Log in: ssh asks for the password, with the remember box beside the field. */
export const Password: Story = {};

/** A refused password asks again with an empty field and says why. */
export const WrongPassword: Story = {
	args: { machines: source({ ...password, notice: 'Wrong password. Try again.', question: 2 }) },
};

/** A first connection: the new host's whole fingerprint, and yes typed to trust it. */
export const NewHostChinese: Story = {
	args: {
		machines: source({
			asked: [
				'新主机 dev@10.0.0.8，ED25519 密钥：',
				'SHA256:p2QAMXNIC1TJYWeIOttrVc98/R1BUFWu3/LiyKgUfQM',
				'输入 yes 以信任：',
			],
			ended: null,
			kind: 'host-key',
			labels: CHINESE_LABELS,
			login: 2,
			machine: 'devbox · dev@10.0.0.8',
			notice: null,
			question: 1,
			remember: null,
			rememberLine: null,
			said: [],
			sending: false,
		}),
	},
};

/** A login that ended without the person keeps the Dialog, without the field, until Close. */
export const Ended: Story = {
	args: {
		machines: source({
			...password,
			ended: { succeeded: false, text: 'Login failed. Choose the machine to try again.' },
		}),
	},
};
