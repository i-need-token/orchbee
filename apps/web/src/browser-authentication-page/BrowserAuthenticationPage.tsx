import { browserAuthenticationMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	Button,
	Card,
	CardContent,
	CardDescription,
	CardHeader,
	CardTitle,
	Field,
	FieldDescription,
	FieldError,
	FieldLabel,
	Input,
	ProductIcon,
} from '@orchbee/ui/components';
import type { FormEvent, ReactNode } from 'react';

import { BROWSER_AUTHENTICATION_ERROR_MESSAGES } from './constants/browser-authentication-error-messages';
import { useBrowserAuthentication } from './hooks/useBrowserAuthentication';
import type { BrowserAuthenticationControllerPort } from './model/browser-authentication-controller';
import styles from './BrowserAuthenticationPage.module.css';

interface BrowserAuthenticationPageProps {
	readonly controller: BrowserAuthenticationControllerPort;
}

export function BrowserAuthenticationPage({
	controller,
}: BrowserAuthenticationPageProps): ReactNode {
	const intl = useOrchbeeIntl();
	const state = useBrowserAuthentication(controller);
	const errorId = 'browser-authentication-error';
	const securityNoticeId = 'browser-authentication-security-notice';
	const handleSubmit = (event: FormEvent<HTMLFormElement>): void => {
		event.preventDefault();
		controller.submit();
	};

	return (
		<main className={styles['page']} data-slot='browser-authentication-page'>
			<Card className={styles['card']}>
				<CardHeader>
					<div className={styles['identity']} aria-hidden='true'>
						<ProductIcon name='terminal' />
						<span>Orchbee</span>
					</div>
					<CardTitle className={styles['title']}>
						{intl.formatMessage(browserAuthenticationMessages.title)}
					</CardTitle>
					<CardDescription className={styles['description']}>
						{intl.formatMessage(browserAuthenticationMessages.description)}
					</CardDescription>
				</CardHeader>
				<CardContent>
					<form className={styles['form']} onSubmit={handleSubmit}>
						<Field data-invalid={state.error === null ? undefined : true}>
							<FieldLabel className={styles['label']} htmlFor='personal-access-token'>
								{intl.formatMessage(browserAuthenticationMessages.personalAccessTokenLabel)}
							</FieldLabel>
							<Input
								aria-describedby={
									state.error === null ? securityNoticeId : `${securityNoticeId} ${errorId}`
								}
								aria-invalid={state.error === null ? undefined : true}
								autoCapitalize='none'
								autoComplete='off'
								autoCorrect='off'
								autoFocus
								className={styles['token']}
								disabled={state.pending}
								id='personal-access-token'
								name='personal-access-token'
								placeholder={intl.formatMessage(
									browserAuthenticationMessages.personalAccessTokenPlaceholder,
								)}
								required
								spellCheck={false}
								type='password'
								value={state.personalAccessToken}
								onChange={(event) => controller.setPersonalAccessToken(event.currentTarget.value)}
							/>
							<FieldDescription className={styles['supporting']} id={securityNoticeId}>
								{intl.formatMessage(browserAuthenticationMessages.securityNotice)}
							</FieldDescription>
							{state.error === null ? null : (
								<FieldError className={styles['error']} id={errorId}>
									{intl.formatMessage(BROWSER_AUTHENTICATION_ERROR_MESSAGES[state.error])}
								</FieldError>
							)}
						</Field>
						<Button
							className={styles['submit']}
							data-browser-authentication-action='submit'
							disabled={state.pending}
							type='submit'
						>
							{intl.formatMessage(
								state.pending
									? browserAuthenticationMessages.submitting
									: browserAuthenticationMessages.submit,
							)}
						</Button>
					</form>
				</CardContent>
			</Card>
		</main>
	);
}
