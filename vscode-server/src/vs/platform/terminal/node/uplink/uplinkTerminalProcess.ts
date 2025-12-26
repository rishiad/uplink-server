/*---------------------------------------------------------------------------------------------
 * UplinkTerminalProcess: Adapter that wraps UplinkPtyClient to implement ITerminalChildProcess
 * This allows the Rust PTY service to be used as a drop-in replacement for node-pty
 *--------------------------------------------------------------------------------------------*/

import { Emitter } from '../../../../base/common/event.js';
import { Disposable } from '../../../../base/common/lifecycle.js';
import { IProcessEnvironment } from '../../../../base/common/platform.js';
import { ILogService } from '../../../log/common/log.js';
import {
	IShellLaunchConfig,
	ITerminalChildProcess,
	ITerminalLaunchError,
	IProcessProperty,
	IProcessPropertyMap,
	ProcessPropertyType,
	IProcessReadyEvent,
	TerminalShellType,
	ITerminalLaunchResult
} from '../../common/terminal.js';
import { UplinkPtyClient, CreatedResponse } from './uplinkPtyClient.js';

const UPLINK_SOCKET_PATH = '/tmp/uplink-pty.sock';

// Singleton client shared by all terminal instances
let sharedClient: UplinkPtyClient | null = null;
let sharedClientPromise: Promise<UplinkPtyClient> | null = null;

async function getSharedClient(): Promise<UplinkPtyClient> {
	if (sharedClient) {
		return sharedClient;
	}
	if (sharedClientPromise) {
		return sharedClientPromise;
	}
	sharedClientPromise = (async () => {
		const client = new UplinkPtyClient(UPLINK_SOCKET_PATH);
		await client.connect();
		sharedClient = client;
		return client;
	})();
	return sharedClientPromise;
}

export class UplinkTerminalProcess extends Disposable implements ITerminalChildProcess {
	readonly id = 0;
	readonly shouldPersist = false;

	private _client: UplinkPtyClient | null = null;
	private _terminalId: number = 0;
	private _pid: number = 0;
	private _currentTitle: string = '';

	get currentTitle(): string { return this._currentTitle; }
	get shellType(): TerminalShellType | undefined { return undefined; }
	get hasChildProcesses(): boolean { return true; }
	get exitMessage(): string | undefined { return undefined; }

	private readonly _onProcessData = this._register(new Emitter<string>());
	readonly onProcessData = this._onProcessData.event;
	private readonly _onProcessReady = this._register(new Emitter<IProcessReadyEvent>());
	readonly onProcessReady = this._onProcessReady.event;
	private readonly _onDidChangeProperty = this._register(new Emitter<IProcessProperty<any>>());
	readonly onDidChangeProperty = this._onDidChangeProperty.event;
	private readonly _onProcessExit = this._register(new Emitter<number>());
	readonly onProcessExit = this._onProcessExit.event;

	private _dataHandler = (terminalId: number, data: Buffer) => {
		if (terminalId === this._terminalId) {
			this._onProcessData.fire(data.toString());
		}
	};

	private _exitHandler = (terminalId: number, code: number | null) => {
		if (terminalId === this._terminalId) {
			this._onProcessExit.fire(code ?? 0);
		}
	};

	constructor(
		readonly shellLaunchConfig: IShellLaunchConfig,
		private readonly _cwd: string,
		private readonly _cols: number,
		private readonly _rows: number,
		private readonly _env: IProcessEnvironment,
		private readonly _logService: ILogService
	) {
		super();
	}

	async start(): Promise<ITerminalLaunchError | ITerminalLaunchResult | undefined> {
		try {
			this._client = await getSharedClient();
		} catch (err) {
			return { message: `Failed to connect to uplink-pty socket: ${(err as Error).message}` };
		}

		this._client.on('data', this._dataHandler);
		this._client.on('exit', this._exitHandler);

		try {
			const result: CreatedResponse = await this._client.create({
				shell: this.shellLaunchConfig.executable || '/bin/bash',
				args: (this.shellLaunchConfig.args as string[]) || [],
				cwd: this._cwd,
				env: this._env as Record<string, string>,
				cols: this._cols,
				rows: this._rows
			});

			this._terminalId = result.terminal_id;
			this._pid = result.pid;
			this._currentTitle = this.shellLaunchConfig.executable || 'terminal';

			this._onProcessReady.fire({
				pid: this._pid,
				cwd: this._cwd,
				windowsPty: undefined
			});

			return undefined;
		} catch (err) {
			return { message: `Failed to create terminal process: ${(err as Error).message}` };
		}
	}

	shutdown(immediate: boolean): void {
		if (this._client) {
			this._client.off('data', this._dataHandler);
			this._client.off('exit', this._exitHandler);
			this._client.kill(this._terminalId).catch(() => {});
		}
	}

	input(data: string): void {
		this._client?.input(this._terminalId, Buffer.from(data)).catch(err => {
			this._logService.error('UplinkTerminalProcess input error:', err);
		});
	}

	resize(cols: number, rows: number): void {
		this._client?.resize(this._terminalId, cols, rows).catch(err => {
			this._logService.error('UplinkTerminalProcess resize error:', err);
		});
	}

	// Stub implementations for interface compliance
	async processBinary(data: string): Promise<void> { this.input(data); }
	acknowledgeDataEvent(charCount: number): void {}
	async setUnicodeVersion(version: '6' | '11'): Promise<void> {}
	getInitialCwd(): Promise<string> { return Promise.resolve(this._cwd); }
	getCwd(): Promise<string> { return Promise.resolve(this._cwd); }
	async refreshProperty<T extends ProcessPropertyType>(type: T): Promise<IProcessPropertyMap[T]> {
		return this._cwd as IProcessPropertyMap[T];
	}
	async updateProperty<T extends ProcessPropertyType>(type: T, value: IProcessPropertyMap[T]): Promise<void> {}
	clearUnacknowledgedChars(): void {}
	clearBuffer(): void {}
	getWindowsPty(): undefined { return undefined; }
	sendSignal(signal: string): void {}
}
