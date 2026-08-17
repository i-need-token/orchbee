export const SHELL_INTEGRATION_SETUP_COMMANDS = Object.freeze({
	bash: 'test -r "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.bash" && source "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.bash"',
	fish: 'test -r "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.fish"; and source "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.fish"',
	zsh: 'test -r "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.zsh" && source "$ORCHBEE_SHELL_INTEGRATION_DIR/orchbee.zsh"',
});
