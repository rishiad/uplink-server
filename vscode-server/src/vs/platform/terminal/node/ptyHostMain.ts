/*---------------------------------------------------------------------------------------------
 *  Copyright (c) Microsoft Corporation. All rights reserved.
 *  Licensed under the MIT License. See License.txt in the project root for license information.
 *--------------------------------------------------------------------------------------------*/

import { DefaultURITransformer } from '../../../base/common/uriIpc.js';
import { ProxyChannel } from '../../../base/parts/ipc/common/ipc.js';
import { Server as ChildProcessServer } from '../../../base/parts/ipc/node/ipc.cp.js';
import { Server as UtilityProcessServer } from '../../../base/parts/ipc/node/ipc.mp.js';
import { localize } from '../../../nls.js';
import { OPTIONS, parseArgs } from '../../environment/node/argv.js';
import { NativeEnvironmentService } from '../../environment/node/environmentService.js';
import { getLogLevel } from '../../log/common/log.js';
import { LoggerChannel } from '../../log/common/logIpc.js';
import { LogService } from '../../log/common/logService.js';
import { LoggerService } from '../../log/node/loggerService.js';
import product from '../../product/common/product.js';
import { IProductService } from '../../product/common/productService.js';
import { IReconnectConstants, TerminalIpcChannels } from '../common/terminal.js';
import { HeartbeatService } from './heartbeatService.js';
import { PtyService } from './ptyService.js';
import { isUtilityProcess } from '../../../base/parts/sandbox/node/electronTypes.js';
import { timeout } from '../../../base/common/async.js';
import { DisposableStore } from '../../../base/common/lifecycle.js';
import { ChildProcess, spawn } from 'child_process';
import * as path from 'path';
import * as fs from 'fs';
import { fileURLToPath } from 'url';

console.log('[ptyHostMain] Starting...');
startPtyHost();

let uplinkPtyProcess: ChildProcess | null = null;

/** Start the uplink-pty Rust service */
async function startUplinkPty(logService: { info: (msg: string) => void; error: (msg: string, err?: any) => void }): Promise<void> {
	// Find uplink-pty binary: check env var first, then fall back to relative path
	let uplinkPtyPath = process.env.UPLINK_PTY_PATH;

	if (!uplinkPtyPath) {
		// Fall back to relative path from build output
		const currentFile = fileURLToPath(import.meta.url);
		const currentDir = path.dirname(currentFile);
		const serverRoot = path.resolve(currentDir, '../../../../..');
		uplinkPtyPath = path.join(serverRoot, 'bin', 'uplink-pty');
	}

	console.log(`[uplink-pty] Binary path: ${uplinkPtyPath}`);
	console.log(`[uplink-pty] Binary exists: ${fs.existsSync(uplinkPtyPath)}`);

	if (!fs.existsSync(uplinkPtyPath)) {
		throw new Error(`uplink-pty binary not found at ${uplinkPtyPath}`);
	}

	return new Promise((resolve, reject) => {
		let timeoutId: NodeJS.Timeout | null = null;

		uplinkPtyProcess = spawn(uplinkPtyPath, [], {
			stdio: ['ignore', 'pipe', 'pipe'],
			detached: false
		});

		uplinkPtyProcess.on('error', (err) => {
			if (timeoutId) {
				clearTimeout(timeoutId);
				timeoutId = null;
			}
			reject(new Error(`Failed to start uplink-pty: ${err.message}`));
		});

		uplinkPtyProcess.on('exit', (code) => {
			logService.error(`uplink-pty exited with code ${code}`);
		});

		// Wait for "listening" message or timeout
		uplinkPtyProcess.stdout?.on('data', (data: Buffer) => {
			const msg = data.toString();
			logService.info(`[uplink-pty] ${msg.trim()}`);
			if (timeoutId && msg.includes('listening')) {
				clearTimeout(timeoutId);
				timeoutId = null;
				resolve();
			}
		});

		uplinkPtyProcess.stderr?.on('data', (data: Buffer) => {
			logService.error(`[uplink-pty] ${data.toString().trim()}`);
		});

		// Timeout after 5 seconds
		timeoutId = setTimeout(() => {
			timeoutId = null;
			reject(new Error('uplink-pty failed to start within 5 seconds'));
		}, 5000);
	});
}

async function startPtyHost() {
	console.log('[ptyHostMain] startPtyHost called');
	// Parse environment variables
	const startupDelay = parseInt(process.env.VSCODE_STARTUP_DELAY ?? '0');
	const simulatedLatency = parseInt(process.env.VSCODE_LATENCY ?? '0');
	const reconnectConstants: IReconnectConstants = {
		graceTime: parseInt(process.env.VSCODE_RECONNECT_GRACE_TIME || '0'),
		shortGraceTime: parseInt(process.env.VSCODE_RECONNECT_SHORT_GRACE_TIME || '0'),
		scrollback: parseInt(process.env.VSCODE_RECONNECT_SCROLLBACK || '100')
	};

	// Sanitize environment
	delete process.env.VSCODE_RECONNECT_GRACE_TIME;
	delete process.env.VSCODE_RECONNECT_SHORT_GRACE_TIME;
	delete process.env.VSCODE_RECONNECT_SCROLLBACK;
	delete process.env.VSCODE_LATENCY;
	delete process.env.VSCODE_STARTUP_DELAY;

	// Delay startup if needed, this must occur before RPC is setup to avoid the channel from timing
	// out.
	if (startupDelay) {
		await timeout(startupDelay);
	}

	// Setup RPC
	const _isUtilityProcess = isUtilityProcess(process);
	let server: ChildProcessServer<string> | UtilityProcessServer;
	if (_isUtilityProcess) {
		server = new UtilityProcessServer();
	} else {
		server = new ChildProcessServer(TerminalIpcChannels.PtyHost);
	}

	// Services
	const productService: IProductService = { _serviceBrand: undefined, ...product };
	const environmentService = new NativeEnvironmentService(parseArgs(process.argv, OPTIONS), productService);
	const loggerService = new LoggerService(getLogLevel(environmentService), environmentService.logsHome);
	server.registerChannel(TerminalIpcChannels.Logger, new LoggerChannel(loggerService, () => DefaultURITransformer));
	const logger = loggerService.createLogger('ptyhost', { name: localize('ptyHost', "Pty Host") });
	const logService = new LogService(logger);

	// Log developer config
	if (startupDelay) {
		logService.warn(`Pty Host startup is delayed ${startupDelay}ms`);
	}
	if (simulatedLatency) {
		logService.warn(`Pty host is simulating ${simulatedLatency}ms latency`);
	}

	const disposables = new DisposableStore();

	// Start uplink-pty Rust service
	console.log('[ptyHostMain] About to start uplink-pty');
	try {
		await startUplinkPty(logService);
		console.log('[ptyHostMain] uplink-pty started successfully');
	} catch (err) {
		console.error('[ptyHostMain] Failed to start uplink-pty:', err);
		logService.error('Failed to start uplink-pty:', err);
		process.exit(1);
	}

	// Heartbeat responsiveness tracking
	const heartbeatService = new HeartbeatService();
	server.registerChannel(TerminalIpcChannels.Heartbeat, ProxyChannel.fromService(heartbeatService, disposables));

	// Init pty service
	const ptyService = new PtyService(logService, reconnectConstants, simulatedLatency);
	const ptyServiceChannel = ProxyChannel.fromService(ptyService, disposables);
	server.registerChannel(TerminalIpcChannels.PtyHost, ptyServiceChannel);

	// Register a channel for direct communication via Message Port
	if (_isUtilityProcess) {
		server.registerChannel(TerminalIpcChannels.PtyHostWindow, ptyServiceChannel);
	}

	// Clean up
	process.once('exit', () => {
		logService.trace('Pty host exiting');
		if (uplinkPtyProcess) {
			uplinkPtyProcess.kill();
		}
		logService.dispose();
		heartbeatService.dispose();
		ptyService.dispose();
	});
}
