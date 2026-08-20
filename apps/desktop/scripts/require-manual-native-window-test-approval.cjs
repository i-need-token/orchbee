const APPROVAL_ENVIRONMENT_VARIABLE = 'ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL';
const REQUIRED_APPROVAL = 'I_ACKNOWLEDGE_FOCUS_INTERRUPTION';

module.exports = function requireManualNativeWindowTestApproval() {
	if (process.env[APPROVAL_ENVIRONMENT_VARIABLE] === REQUIRED_APPROVAL) return;
	throw new Error(
		`The manual native-window test changes operating-system focus. Set ${APPROVAL_ENVIRONMENT_VARIABLE}=${REQUIRED_APPROVAL} only after a human explicitly approves this exact test.`,
	);
};
