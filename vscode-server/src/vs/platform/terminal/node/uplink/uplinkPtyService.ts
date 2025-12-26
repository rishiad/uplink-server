/*---------------------------------------------------------------------------------------------
 * UplinkPtyService: Replaces PtyHostService by talking directly to Rust PTY server
 * This eliminates the Node.js PTY host process entirely
 *--------------------------------------------------------------------------------------------*/

import { Emitter } from '../../../../base/common/event.js';
import { Disposable } from '../../../../base/common/lifecycle.js';
import { IProcessEnvironment, OperatingSystem, OS } from '../../../../base/common/platform.js';
import { getSystemShell } from '../../../../base/node/shell.js';
import { IPtyHostProcessReplayEvent } from '../../common/capabilities/capabilities.js';
import {
	IPtyHostService,
	IShellLaunchConfig,
	ITerminalLaunchError,
	ITerminalLaunchResult,
	IProcessDataEvent,
	IProcessProperty,
	IProcessPropertyMap,
	ProcessPropertyType,
	IProcessReadyEvent,
	IPtyHostLatencyMeasurement,
	ITerminalProcessOptions,
	ITerminalsLayoutInfo,
	ITerminalProfile,
	TitleEventSource,
	TerminalIcon,
	IRequestResolveVariablesEvent,
	ISerializedTerminalState,
} from '../../common/terminal.js';
import { IProcessDetails, IGetTerminalLayoutInfoArgs, ISetTerminalLayoutInfoArgs } from '../../common/terminalProcess.js';
import { UplinkPtyClient } from './uplinkPtyClient.js';

const UPLINK_SOCKET_PATH = '/tmp/uplink-pty.sock';

interface TerminalState {
	cwd: string;
	initialCwd: string;
	title: string;
	pid: number;
}

export class UplinkPtyService extends Disposable implements IPtyHostService {
	declare readonly _serviceBrand: undefined;

	private _client: UplinkPtyClient | null = null;
	private _clientPromise: Promise<UplinkPtyClient> | null = null;
	private _nextId = 1;
	private _terminals = new Map<number, { terminalId: number; state: TerminalState }>();

	// Events required by IPtyService
	private readonly _onProcessData = this._register(new Emitter<{ id: number; event: IProcessDataEvent | string }>());
	readonly onProcessData = this._onProcessData.event;
	private readonly _onProcessReady = this._register(new Emitter<{ id: number; event: IProcessReadyEvent }>());
	readonly onProcessReady = this._onProcessReady.event;
	private readonly _onProcessReplay = this._register(new Emitter<{ id: number; event: IPtyHostProcessReplayEvent }>());
	readonly onProcessReplay = this._onProcessReplay.event;
	private readonly _onProcessOrphanQuestion = this._register(new Emitter<{ id: number }>());
	readonly onProcessOrphanQuestion = this._onProcessOrphanQuestion.event;
	private readonly _onDidRequestDetach = this._register(new Emitter<{ requestId: number; workspaceId: string; instanceId: number }>());
	readonly onDidRequestDetach = this._onDidRequestDetach.event;
	private readonly _onDidChangeProperty = this._register(new Emitter<{ id: number; property: IProcessProperty<any> }>());
	readonly onDidChangeProperty = this._onDidChangeProperty.event;
	private readonly _onProcessExit = this._register(new Emitter<{ id: number; event: number | undefined }>());
	readonly onProcessExit = this._onProcessExit.event;

	// Events required by IPtyHostController
	private readonly _onPtyHostExit = this._register(new Emitter<number>());
	readonly onPtyHostExit = this._onPtyHostExit.event;
	private readonly _onPtyHostStart = this._register(new Emitter<void>());
	readonly onPtyHostStart = this._onPtyHostStart.event;
	private readonly _onPtyHostUnresponsive = this._register(new Emitter<void>());
	readonly onPtyHostUnresponsive = this._onPtyHostUnresponsive.event;
	private readonly _onPtyHostResponsive = this._register(new Emitter<void>());
	readonly onPtyHostResponsive = this._onPtyHostResponsive.event;
	private readonly _onPtyHostRequestResolveVariables = this._register(new Emitter<IRequestResolveVariablesEvent>());
	readonly onPtyHostRequestResolveVariables = this._onPtyHostRequestResolveVariables.event;

	private async _getClient(): Promise<UplinkPtyClient> {
		if (this._client) {
			return this._client;
		}
		if (this._clientPromise) {
			return this._clientPromise;
		}
		this._clientPromise = (async () => {
			const client = new UplinkPtyClient(UPLINK_SOCKET_PATH);
			await client.connect();

			client.on('data', (terminalId: number, data: Buffer) => {
				console.log(`[UplinkPtyService] data event terminalId=${terminalId} bytes=${data.length}`);
				const entry = this._findByTerminalId(terminalId);
				if (entry) {
					console.log(`[UplinkPtyService] firing onProcessData id=${entry.id}`);
					this._onProcessData.fire({ id: entry.id, event: data.toString() });
				} else {
					console.log(`[UplinkPtyService] data: no entry for terminalId=${terminalId}`);
				}
			});

			client.on('exit', (terminalId: number, code: number | null) => {
				const entry = this._findByTerminalId(terminalId);
				if (entry) {
					this._onProcessExit.fire({ id: entry.id, event: code ?? undefined });
					this._terminals.delete(entry.id);
				}
			});

			client.on('error', () => this._onPtyHostUnresponsive.fire());
			client.on('close', () => {
				this._client = null;
				this._clientPromise = null;
			});

			this._client = client;
			this._onPtyHostStart.fire();
			return client;
		})();
		return this._clientPromise;
	}

	private _findByTerminalId(terminalId: number): { id: number; terminalId: number; state: TerminalState } | undefined {
		for (const [id, entry] of this._terminals) {
			if (entry.terminalId === terminalId) {
				return { id, ...entry };
			}
		}
		return undefined;
	}

	async createProcess(
		shellLaunchConfig: IShellLaunchConfig,
		cwd: string,
		cols: number,
		rows: number,
		_unicodeVersion: '6' | '11',
		env: IProcessEnvironment,
		_executableEnv: IProcessEnvironment,
		_options: ITerminalProcessOptions,
		_shouldPersist: boolean,
		_workspaceId: string,
		_workspaceName: string
	): Promise<number> {
		console.log(`[UplinkPtyService] createProcess shell=${shellLaunchConfig.executable} cwd=${cwd}`);
		const client = await this._getClient();
		const id = this._nextId++;

		const result = await client.create({
			shell: shellLaunchConfig.executable || '/bin/bash',
			args: (shellLaunchConfig.args as string[]) || [],
			cwd,
			env: env as Record<string, string>,
			cols,
			rows
		});

		this._terminals.set(id, {
			terminalId: result.terminal_id,
			state: { cwd, initialCwd: cwd, title: shellLaunchConfig.name || shellLaunchConfig.executable || 'terminal', pid: result.pid }
		});

		console.log(`[UplinkPtyService] createProcess: id=${id} terminalId=${result.terminal_id} pid=${result.pid}`);
		return id;
	}

	async start(id: number): Promise<ITerminalLaunchError | ITerminalLaunchResult | undefined> {
		console.log(`[UplinkPtyService] start id=${id}`);
		const entry = this._terminals.get(id);
		if (!entry) {
			console.log(`[UplinkPtyService] start: terminal ${id} not found`);
			return { message: 'Terminal not found' };
		}
		// Terminal already started in createProcess, fire ready event
		console.log(`[UplinkPtyService] start: firing onProcessReady for id=${id} pid=${entry.state.pid}`);
		this._onProcessReady.fire({
			id,
			event: { pid: entry.state.pid, cwd: entry.state.cwd, windowsPty: undefined }
		});
		return undefined;
	}

	async input(id: number, data: string): Promise<void> {
		console.log(`[UplinkPtyService] input id=${id} data.length=${data?.length}`);
		const entry = this._terminals.get(id);
		if (!entry) {
			console.log(`[UplinkPtyService] input: terminal ${id} not found`);
			return;
		}
		console.log(`[UplinkPtyService] input: sending to terminalId=${entry.terminalId}`);
		const client = await this._getClient();
		await client.input(entry.terminalId, Buffer.from(data));
	}

	async resize(id: number, cols: number, rows: number): Promise<void> {
		const entry = this._terminals.get(id);
		if (!entry) return;
		const client = await this._getClient();
		await client.resize(entry.terminalId, cols, rows);
	}

	async shutdown(id: number, _immediate: boolean): Promise<void> {
		const entry = this._terminals.get(id);
		if (!entry) return;
		const client = await this._getClient();
		await client.kill(entry.terminalId);
		this._terminals.delete(id);
	}

	async shutdownAll(): Promise<void> {
		for (const [id] of this._terminals) {
			await this.shutdown(id, true);
		}
	}

	// Stub implementations for less critical methods
	async attachToProcess(_id: number): Promise<void> {}
	async detachFromProcess(_id: number, _forcePersist?: boolean): Promise<void> {}
	async listProcesses(): Promise<IProcessDetails[]> { return []; }
	async getPerformanceMarks(): Promise<any[]> { return []; }
	async getLatency(): Promise<IPtyHostLatencyMeasurement[]> { return [{ label: 'uplink-pty', latency: 0 }]; }
	async sendSignal(_id: number, _signal: string): Promise<void> {}
	async clearBuffer(_id: number): Promise<void> {}
	async getInitialCwd(id: number): Promise<string> {
		return this._terminals.get(id)?.state.initialCwd || '';
	}
	async getCwd(id: number): Promise<string> {
		return this._terminals.get(id)?.state.cwd || '';
	}
	async acknowledgeDataEvent(_id: number, _charCount: number): Promise<void> {}
	async setUnicodeVersion(_id: number, _version: '6' | '11'): Promise<void> {}
	async processBinary(id: number, data: string): Promise<void> { await this.input(id, data); }
	async orphanQuestionReply(_id: number): Promise<void> {}
	async updateTitle(_id: number, _title: string, _titleSource: TitleEventSource): Promise<void> {}
	async updateIcon(_id: number, _userInitiated: boolean, _icon: TerminalIcon, _color?: string): Promise<void> {}
	async getDefaultSystemShell(osOverride?: OperatingSystem): Promise<string> {
		return getSystemShell(osOverride ?? OS, process.env);
	}
	async getEnvironment(): Promise<IProcessEnvironment> { return { ...process.env }; }
	async getWslPath(original: string, _direction: 'unix-to-win' | 'win-to-unix'): Promise<string> { return original; }
	async getRevivedPtyNewId(_workspaceId: string, _id: number): Promise<number | undefined> { return undefined; }
	async setTerminalLayoutInfo(_args: ISetTerminalLayoutInfoArgs): Promise<void> {}
	async getTerminalLayoutInfo(_args: IGetTerminalLayoutInfoArgs): Promise<ITerminalsLayoutInfo | undefined> { return undefined; }
	async reduceConnectionGraceTime(): Promise<void> {}
	async requestDetachInstance(_workspaceId: string, _instanceId: number): Promise<IProcessDetails | undefined> { return undefined; }
	async acceptDetachInstanceReply(_requestId: number, _persistentProcessId?: number): Promise<void> {}
	async freePortKillProcess(_port: string): Promise<{ port: string; processId: string }> { return { port: '', processId: '' }; }
	async serializeTerminalState(_ids: number[]): Promise<string> { return ''; }
	async reviveTerminalProcesses(_workspaceId: string, _state: ISerializedTerminalState[], _dateTimeFormatLocate: string): Promise<void> {}
	async refreshProperty<T extends ProcessPropertyType>(id: number, property: T): Promise<IProcessPropertyMap[T]> {
		const entry = this._terminals.get(id);
		if (property === ProcessPropertyType.Cwd) return (entry?.state.cwd || '') as IProcessPropertyMap[T];
		if (property === ProcessPropertyType.InitialCwd) return (entry?.state.initialCwd || '') as IProcessPropertyMap[T];
		return '' as IProcessPropertyMap[T];
	}
	async updateProperty<T extends ProcessPropertyType>(_id: number, _property: T, _value: IProcessPropertyMap[T]): Promise<void> {}
	async refreshIgnoreProcessNames(_names: string[]): Promise<void> {}
	async installAutoReply(_match: string, _reply: string): Promise<void> {}
	async uninstallAllAutoReplies(): Promise<void> {}

	// IPtyHostController methods
	async restartPtyHost(): Promise<void> {
		this._client?.close();
		this._client = null;
		this._clientPromise = null;
		await this._getClient();
	}
	async acceptPtyHostResolvedVariables(_requestId: number, _resolved: string[]): Promise<void> {}
	async getProfiles(_workspaceId: string, _profiles: unknown, _defaultProfile: unknown, _includeDetectedProfiles?: boolean): Promise<ITerminalProfile[]> {
		return [];
	}
}
