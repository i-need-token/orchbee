import { OrchbeeI18nProvider, type OrchbeeLocale } from '@orchbee/i18n';
import type { Decorator, Meta, StoryObj } from 'storybook-react-rsbuild';

import { SessionUnavailable } from '../src/desktop-client/session-unavailable';

const meta = {
	title: 'Desktop/Session Unavailable',
	component: SessionUnavailable,
	args: { onTryAgain: () => undefined, refusal: null },
	parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof SessionUnavailable>;

export default meta;

type Story = StoryObj<typeof meta>;

function inLocale(locale: OrchbeeLocale): Decorator {
	return (StoryComponent) => (
		<OrchbeeI18nProvider locale={locale}>
			<StoryComponent />
		</OrchbeeI18nProvider>
	);
}

const english = [inLocale('en-US')];
const simplifiedChinese = [inLocale('zh-CN')];

const desktopTooOld = {
	desktopVersion: '1.2.0',
	reason: 'desktop-too-old',
	sessionVersion: '1.6.0',
} as const;
const sessionTooOld = {
	desktopVersion: '1.6.0',
	reason: 'session-too-old',
	sessionVersion: '1.2.0',
} as const;
const otherEnvironment = { reason: 'other-environment' } as const;
const developmentBuildMismatch = { reason: 'development-build-mismatch' } as const;

export const Unavailable: Story = { decorators: english };

export const RecoveryRequired: Story = {
	args: { refusal: { reason: 'recovery-required' } },
	decorators: english,
};

export const DesktopTooOld: Story = { args: { refusal: desktopTooOld }, decorators: english };

export const SessionTooOld: Story = { args: { refusal: sessionTooOld }, decorators: english };

/** A Session older than the handshake states no version, so the message leaves it out. */
export const SessionTooOldWithoutVersion: Story = {
	args: { refusal: { ...sessionTooOld, sessionVersion: null } },
	decorators: english,
};

export const OtherEnvironment: Story = { args: { refusal: otherEnvironment }, decorators: english };

export const DevelopmentBuildMismatch: Story = {
	args: { refusal: developmentBuildMismatch },
	decorators: english,
};

export const SimplifiedChinese: Story = {
	args: { refusal: { reason: 'recovery-required' } },
	decorators: simplifiedChinese,
};

export const SimplifiedChineseDesktopTooOld: Story = {
	args: { refusal: desktopTooOld },
	decorators: simplifiedChinese,
};

export const SimplifiedChineseSessionTooOld: Story = {
	args: { refusal: sessionTooOld },
	decorators: simplifiedChinese,
};

export const SimplifiedChineseOtherEnvironment: Story = {
	args: { refusal: otherEnvironment },
	decorators: simplifiedChinese,
};

export const SimplifiedChineseDevelopmentBuildMismatch: Story = {
	args: { refusal: developmentBuildMismatch },
	decorators: simplifiedChinese,
};
