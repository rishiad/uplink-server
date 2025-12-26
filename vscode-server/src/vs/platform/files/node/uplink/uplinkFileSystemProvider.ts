/*---------------------------------------------------------------------------------------------
 * UplinkFileSystemProvider: Proxies filesystem operations to Rust uplink-fs service
 *--------------------------------------------------------------------------------------------*/

import { Emitter, Event } from '../../../../base/common/event.js';
import { Disposable, IDisposable, toDisposable } from '../../../../base/common/lifecycle.js';
import { URI } from '../../../../base/common/uri.js';
import { FileSystemProviderCapabilities, FileSystemProviderErrorCode, FileType, IFileChange, IFileDeleteOptions, IFileOverwriteOptions, IFileSystemProviderWithFileReadWriteCapability, IFileSystemProviderWithFileFolderCopyCapability, IFileWriteOptions, IStat, createFileSystemProviderError, IWatchOptions, IFileAtomicReadOptions, IFileSystemProviderWithFileRealpathCapability } from '../../common/files.js';
import { ILogService } from '../../../log/common/log.js';
import { UplinkFsClient, FileChange } from './uplinkFsClient.js';
import { isLinux } from '../../../../base/common/platform.js';

const SOCKET_PATH = '/tmp/uplink-fs.sock';

export class UplinkFileSystemProvider extends Disposable implements
	IFileSystemProviderWithFileReadWriteCapability,
	IFileSystemProviderWithFileFolderCopyCapability,
	IFileSystemProviderWithFileRealpathCapability {

	readonly onDidChangeCapabilities = Event.None;

	private _capabilities: FileSystemProviderCapabilities | undefined;
	get capabilities(): FileSystemProviderCapabilities {
		if (!this._capabilities) {
			this._capabilities =
				FileSystemProviderCapabilities.FileReadWrite |
				FileSystemProviderCapabilities.FileFolderCopy |
				FileSystemProviderCapabilities.FileRealpath;

			if (isLinux) {
				this._capabilities |= FileSystemProviderCapabilities.PathCaseSensitive;
			}
		}
		return this._capabilities;
	}

	protected readonly _onDidChangeFile = this._register(new Emitter<readonly IFileChange[]>());
	readonly onDidChangeFile = this._onDidChangeFile.event;

	protected readonly _onDidWatchError = this._register(new Emitter<string>());
	readonly onDidWatchError = this._onDidWatchError.event;

	private client: UplinkFsClient | null = null;
	private connecting: Promise<void> | null = null;
	private watchCounter = 0;
	private readonly watches = new Map<number, { sessionId: string; reqId: number }>();

	constructor(
		private readonly logService: ILogService
	) {
		super();
	}

	private async ensureConnected(): Promise<UplinkFsClient> {
		if (this.client) {
			return this.client;
		}

		if (this.connecting) {
			await this.connecting;
			return this.client!;
		}

		this.connecting = this.doConnect();
		await this.connecting;
		this.connecting = null;
		return this.client!;
	}

	private async doConnect(): Promise<void> {
		const client = new UplinkFsClient(SOCKET_PATH);

		client.on('fileChange', (sessionId: string, changes: FileChange[]) => {
			const fileChanges: IFileChange[] = changes.map(c => ({
				type: c.change_type, // 0=Updated, 1=Added, 2=Deleted maps to FileChangeType
				resource: URI.file(c.path),
			}));
			this._onDidChangeFile.fire(fileChanges);
		});

		client.on('watchError', (_sessionId: string, message: string) => {
			this._onDidWatchError.fire(message);
		});

		client.on('error', (err) => {
			this.logService.error('[UplinkFileSystemProvider] Socket error:', err);
			this.client = null;
		});

		client.on('close', () => {
			this.logService.info('[UplinkFileSystemProvider] Socket closed');
			this.client = null;
		});

		await client.connect();
		this.client = client;
		this.logService.info('[UplinkFileSystemProvider] Connected to uplink-fs');
	}

	async stat(resource: URI): Promise<IStat> {
		try {
			const client = await this.ensureConnected();
			const result = await client.stat(resource.fsPath);
			return {
				type: result.type as FileType,
				ctime: result.ctime,
				mtime: result.mtime,
				size: result.size,
			};
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async realpath(resource: URI): Promise<string> {
		try {
			const client = await this.ensureConnected();
			return await client.realpath(resource.fsPath);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async readdir(resource: URI): Promise<[string, FileType][]> {
		try {
			const client = await this.ensureConnected();
			const entries = await client.readDir(resource.fsPath);
			return entries.map(([name, type]) => [name, type as FileType]);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async readFile(resource: URI, _opts?: IFileAtomicReadOptions): Promise<Uint8Array> {
		try {
			const client = await this.ensureConnected();
			return await client.readFile(resource.fsPath);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async writeFile(resource: URI, content: Uint8Array, opts: IFileWriteOptions): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.writeFile(resource.fsPath, content, {
				create: opts.create,
				overwrite: opts.overwrite,
			});
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async mkdir(resource: URI): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.mkdir(resource.fsPath);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async delete(resource: URI, opts: IFileDeleteOptions): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.delete(resource.fsPath, { recursive: opts.recursive });
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async rename(from: URI, to: URI, opts: IFileOverwriteOptions): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.rename(from.fsPath, to.fsPath, { overwrite: opts.overwrite });
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async copy(from: URI, to: URI, opts: IFileOverwriteOptions): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.copy(from.fsPath, to.fsPath, { overwrite: opts.overwrite });
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	watch(resource: URI, opts: IWatchOptions): IDisposable {
		const watchId = this.watchCounter++;
		const sessionId = `session-${process.pid}`;
		const reqId = watchId;

		this.watches.set(watchId, { sessionId, reqId });

		// Start watch asynchronously
		this.ensureConnected().then(client => {
			client.watch(sessionId, reqId, resource.fsPath, opts.recursive).catch(err => {
				this.logService.error('[UplinkFileSystemProvider] Watch error:', err);
			});
		});

		return toDisposable(() => {
			const watch = this.watches.get(watchId);
			if (watch) {
				this.watches.delete(watchId);
				this.ensureConnected().then(client => {
					client.unwatch(watch.sessionId, watch.reqId).catch(() => { });
				});
			}
		});
	}

	private toFileSystemProviderError(error: any): Error {
		const message = error?.message || String(error);

		if (message.includes('No such file') || message.includes('ENOENT') || message.includes('not exist')) {
			return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileNotFound);
		}
		if (message.includes('already exists') || message.includes('EEXIST')) {
			return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileExists);
		}
		if (message.includes('permission') || message.includes('EACCES') || message.includes('EPERM')) {
			return createFileSystemProviderError(message, FileSystemProviderErrorCode.NoPermissions);
		}
		if (message.includes('is a directory') || message.includes('EISDIR')) {
			return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileIsADirectory);
		}

		return createFileSystemProviderError(message, FileSystemProviderErrorCode.Unknown);
	}

	override dispose(): void {
		this.client?.close();
		this.client = null;
		super.dispose();
	}
}
