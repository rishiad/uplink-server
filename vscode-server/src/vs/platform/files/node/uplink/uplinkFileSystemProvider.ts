/*---------------------------------------------------------------------------------------------
 * UplinkFileSystemProvider: Proxies filesystem operations to Rust uplink-fs service
 *--------------------------------------------------------------------------------------------*/

import { Emitter, Event } from '../../../../base/common/event.js';
import { Disposable, IDisposable, toDisposable } from '../../../../base/common/lifecycle.js';
import { URI } from '../../../../base/common/uri.js';
import { FileSystemProviderCapabilities, FileSystemProviderErrorCode, FileType, IFileChange, IFileDeleteOptions, IFileOverwriteOptions, IFileSystemProviderWithFileReadWriteCapability, IFileSystemProviderWithFileFolderCopyCapability, IFileWriteOptions, IStat, createFileSystemProviderError, IWatchOptions, IFileAtomicReadOptions, IFileSystemProviderWithFileRealpathCapability, IFileReadStreamOptions, IFileSystemProviderWithOpenReadWriteCloseCapability, IFileOpenOptions, IFileSystemProviderWithFileCloneCapability } from '../../common/files.js';
import { ILogService } from '../../../log/common/log.js';
import { UplinkFsClient, FileChange } from './uplinkFsClient.js';
import { isLinux } from '../../../../base/common/platform.js';
import { ReadableStreamEvents, newWriteableStream } from '../../../../base/common/stream.js';
import { VSBuffer } from '../../../../base/common/buffer.js';
import { CancellationToken } from '../../../../base/common/cancellation.js';

const SOCKET_PATH = '/tmp/uplink-fs.sock';

export class UplinkFileSystemProvider extends Disposable implements
	IFileSystemProviderWithFileReadWriteCapability,
	IFileSystemProviderWithFileFolderCopyCapability,
	IFileSystemProviderWithFileRealpathCapability,
	IFileSystemProviderWithOpenReadWriteCloseCapability,
	IFileSystemProviderWithFileCloneCapability {

	readonly onDidChangeCapabilities = Event.None;

	private _capabilities: FileSystemProviderCapabilities | undefined;
	get capabilities(): FileSystemProviderCapabilities {
		if (!this._capabilities) {
			this._capabilities =
				FileSystemProviderCapabilities.FileReadWrite |
				FileSystemProviderCapabilities.FileOpenReadWriteClose |
				FileSystemProviderCapabilities.FileFolderCopy |
				FileSystemProviderCapabilities.FileClone |
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
		// Check cache first
		const cached = this.statCache.get(resource.fsPath);
		if (cached && cached.expires > Date.now()) {
			return cached.stat;
		}
		this.statCache.delete(resource.fsPath);

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

	private statCache = new Map<string, { stat: IStat; expires: number }>();
	private readonly CACHE_TTL = 5000; // 5 seconds

	async readdir(resource: URI): Promise<[string, FileType][]> {
		try {
			const client = await this.ensureConnected();
			const entries = await client.readDirWithStats(resource.fsPath);
			const now = Date.now();
			const basePath = resource.fsPath;

			// Cache stat results for each entry
			for (const e of entries) {
				const fullPath = basePath.endsWith('/') ? basePath + e.name : basePath + '/' + e.name;
				this.statCache.set(fullPath, {
					stat: { type: e.type as FileType, ctime: e.ctime, mtime: e.mtime, size: e.size },
					expires: now + this.CACHE_TTL,
				});
			}

			return entries.map(e => [e.name, e.type as FileType]);
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

	readFileStream(resource: URI, opts: IFileReadStreamOptions, token: CancellationToken): ReadableStreamEvents<Uint8Array> {
		const stream = newWriteableStream<Uint8Array>(data => VSBuffer.concat(data.map(d => VSBuffer.wrap(d))).buffer);

		this.doReadFileStream(resource, opts, token, stream);

		return stream;
	}

	private async doReadFileStream(resource: URI, opts: IFileReadStreamOptions, token: CancellationToken, stream: ReturnType<typeof newWriteableStream<Uint8Array>>): Promise<void> {
		try {
			if (token.isCancellationRequested) {
				stream.end();
				return;
			}

			const client = await this.ensureConnected();
			const data = await client.readFile(resource.fsPath);

			if (token.isCancellationRequested) {
				stream.end();
				return;
			}

			// Handle position/length options
			let result = data;
			if (opts.position !== undefined || opts.length !== undefined) {
				const start = opts.position ?? 0;
				const end = opts.length !== undefined ? start + opts.length : data.length;
				result = data.slice(start, end);
			}

			stream.write(result);
			stream.end();
		} catch (error) {
			stream.error(this.toFileSystemProviderError(error));
			stream.end();
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

	async cloneFile(from: URI, to: URI): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.cloneFile(from.fsPath, to.fsPath);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	// File handle operations for streaming
	async open(resource: URI, opts: IFileOpenOptions): Promise<number> {
		try {
			const client = await this.ensureConnected();
			return await client.open(resource.fsPath, {
				create: opts.create,
				truncate: opts.create, // truncate when creating for write
			});
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async close(fd: number): Promise<void> {
		try {
			const client = await this.ensureConnected();
			await client.closeHandle(fd);
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async read(fd: number, pos: number, data: Uint8Array, offset: number, length: number): Promise<number> {
		try {
			const client = await this.ensureConnected();
			const result = await client.readHandle(fd, pos, length);
			data.set(result.data, offset);
			return result.bytesRead;
		} catch (error) {
			throw this.toFileSystemProviderError(error);
		}
	}

	async write(fd: number, pos: number, data: Uint8Array, offset: number, length: number): Promise<number> {
		try {
			const client = await this.ensureConnected();
			const chunk = data.subarray(offset, offset + length);
			return await client.writeHandle(fd, pos, chunk);
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
		const code = error?.code;

		// Use structured error code from Rust if available
		if (code) {
			switch (code) {
				case 'FileNotFound':
					return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileNotFound);
				case 'FileExists':
					return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileExists);
				case 'NoPermissions':
					return createFileSystemProviderError(message, FileSystemProviderErrorCode.NoPermissions);
				case 'FileIsADirectory':
					return createFileSystemProviderError(message, FileSystemProviderErrorCode.FileIsADirectory);
			}
		}

		// Fallback to message-based detection
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
