import { describe, expect, it } from 'vitest';

import { WorkbenchTerminalBellEffectCoordinator } from '../WorkbenchTerminalBellEffectCoordinator';

describe('WorkbenchTerminalBellEffectCoordinator', () => {
	it('admits each terminal sequence once across attachment delivery order and replay', () => {
		const coordinator = new WorkbenchTerminalBellEffectCoordinator();

		expect(coordinator.admit({ sequence: '7', terminalId: 'terminal-a' })).toBe(true);
		expect(coordinator.admit({ sequence: '7', terminalId: 'terminal-a' })).toBe(false);
		expect(coordinator.admit({ sequence: '9', terminalId: 'terminal-a' })).toBe(true);
		expect(coordinator.admit({ sequence: '8', terminalId: 'terminal-a' })).toBe(true);
		expect(coordinator.admit({ sequence: '8', terminalId: 'terminal-a' })).toBe(false);
		expect(coordinator.admit({ sequence: '7', terminalId: 'terminal-b' })).toBe(true);
	});

	it('fails closed for invalid identity, invalid sequence, overflow, and disposal', () => {
		const coordinator = new WorkbenchTerminalBellEffectCoordinator();

		expect(coordinator.admit({ sequence: '1', terminalId: '' })).toBe(false);
		expect(coordinator.admit({ sequence: '0', terminalId: 'terminal-a' })).toBe(false);
		expect(coordinator.admit({ sequence: '01', terminalId: 'terminal-a' })).toBe(false);
		expect(coordinator.admit({ sequence: '18446744073709551616', terminalId: 'terminal-a' })).toBe(
			false,
		);
		coordinator.dispose();
		expect(coordinator.admit({ sequence: '1', terminalId: 'terminal-a' })).toBe(false);
	});
});
