/*---------------------------------------------------------------------------------------------
 * uplink-fs client: Communicates with Rust filesystem service over Unix socket
 * Wire format: [1 byte tag][4 byte length BE][MessagePack payload]
 *--------------------------------------------------------------------------------------------*/

import * as net from 'net';
import { EventEmitter } from 'events';
import { encode, decode } from '@msgpack/msgpack';

// Message type tags - must match Rust protocol.rs
const MSG_STAT = 1;
const MSG_READ_FILE = 2;
const MSG_WRITE_FILE = 3;
const MSG_DELETE = 4;
const MSG_RENAME = 5;
const MSG_COPY = 6;
const MSG_READ_DIR = 7;
const MSG_MKDIR = 8;
const MSG_WATCH = 9;
const MSG_UNWATCH = 10;
const MSG_REALPATH = 11;

const MSG_OK = 20;
const MSG_ERROR = 21;
const MSG_STAT_RESULT = 22;
const MSG_DATA = 23;
const MSG_DIR_ENTRIES = 24;
const MSG_REALPATH_RESULT = 25;

const MSG_FILE_CHANGE = 30;
const MSG_WATCH_ERROR = 31;

export interface StatResult {
	type: number;
	ctime: number;
	mtime: number;
	size: number;
}

export interface DirEntry {
	name: string;
	file_type: number;
}

export interface FileChange {
	change_type: number;
	path: string;
}

type PendingRequest = {
	resolve: (value: any) => void;
	reject: (error: Error) => void;
};

export class UplinkFsClient extends EventEmitter {
	private socket: net.Socket | null = null;
	private buffer: Buffer = Buffer.alloc(0);
	private nextId = 1;
	private pending = new Map<number, PendingRequest>();

	constructor(private socketPath: string) {
		super();
	}

	async connect(): Promise<void> {
		return new Promise((resolve, reject) => {
			let connected = false;
			this.socket = net.createConnection(this.socketPath, () => {
				connected = true;
				resolve();
			});

			this.socket.on('error', (err) => {
				if (!connected) {
					reject(err);
				} else {
					this.emit('error', err);
				}
			});

			this.socket.on('close', () => {
				this.emit('close');
			});

			this.socket.on('data', (chunk) => {
				this.handleData(chunk);
			});
		});
	}

	private handleData(chunk: Buffer): void {
		this.buffer = Buffer.concat([this.buffer, chunk]);

		while (this.buffer.length >= 5) {
			const tag = this.buffer[0];
			const len = this.buffer.readUInt32BE(1);

			if (this.buffer.length < 5 + len) {
				break;
			}

			const payload = this.buffer.subarray(5, 5 + len);
			this.buffer = this.buffer.subarray(5 + len);

			this.handleMessage(tag, payload);
		}
	}

	private handleMessage(tag: number, payload: Buffer): void {
		const msg = decode(payload) as any;

		switch (tag) {
			case MSG_OK:
			case MSG_STAT_RESULT:
			case MSG_DATA:
			case MSG_DIR_ENTRIES:
			case MSG_REALPATH_RESULT: {
				const pending = this.pending.get(msg.id);
				pending?.resolve(msg);
				this.pending.delete(msg.id);
				break;
			}
			case MSG_ERROR: {
				const pending = this.pending.get(msg.id);
				pending?.reject(new Error(msg.message));
				this.pending.delete(msg.id);
				break;
			}
			case MSG_FILE_CHANGE: {
				this.emit('fileChange', msg.session_id, msg.changes);
				break;
			}
			case MSG_WATCH_ERROR: {
				this.emit('watchError', msg.session_id, msg.message);
				break;
			}
		}
	}

	private send(tag: number, msg: any): void {
		if (!this.socket) {
			throw new Error('Not connected');
		}
		const payload = Buffer.from(encode(msg));
		const header = Buffer.alloc(5);
		header[0] = tag;
		header.writeUInt32BE(payload.length, 1);
		this.socket.write(Buffer.concat([header, payload]));
	}

	private request<T>(tag: number, msg: any, id: number): Promise<T> {
		return new Promise((resolve, reject) => {
			this.pending.set(id, { resolve, reject });
			this.send(tag, msg);
		});
	}

	async stat(path: string): Promise<StatResult> {
		const id = this.nextId++;
		const result = await this.request<any>(MSG_STAT, { id, path }, id);
		return {
			type: result.file_type,
			ctime: result.ctime,
			mtime: result.mtime,
			size: result.size,
		};
	}

	async readFile(path: string): Promise<Uint8Array> {
		const id = this.nextId++;
		const result = await this.request<any>(MSG_READ_FILE, { id, path }, id);
		return new Uint8Array(result.data);
	}

	async writeFile(path: string, data: Uint8Array, opts: { create: boolean; overwrite: boolean }): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_WRITE_FILE, { id, path, data: Array.from(data), ...opts }, id);
	}

	async delete(path: string, opts: { recursive: boolean }): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_DELETE, { id, path, ...opts }, id);
	}

	async rename(oldPath: string, newPath: string, opts: { overwrite: boolean }): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_RENAME, { id, old_path: oldPath, new_path: newPath, ...opts }, id);
	}

	async copy(srcPath: string, destPath: string, opts: { overwrite: boolean }): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_COPY, { id, src_path: srcPath, dest_path: destPath, ...opts }, id);
	}

	async readDir(path: string): Promise<Array<[string, number]>> {
		const id = this.nextId++;
		const result = await this.request<any>(MSG_READ_DIR, { id, path }, id);
		return result.entries.map((e: DirEntry) => [e.name, e.file_type] as [string, number]);
	}

	async mkdir(path: string): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_MKDIR, { id, path }, id);
	}

	async watch(sessionId: string, reqId: number, path: string, recursive: boolean): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_WATCH, { id, session_id: sessionId, req_id: reqId, path, recursive }, id);
	}

	async unwatch(sessionId: string, reqId: number): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_UNWATCH, { id, session_id: sessionId, req_id: reqId }, id);
	}

	async realpath(path: string): Promise<string> {
		const id = this.nextId++;
		const result = await this.request<any>(MSG_REALPATH, { id, path }, id);
		return result.path;
	}

	close(): void {
		this.socket?.destroy();
		this.socket = null;
	}
}
