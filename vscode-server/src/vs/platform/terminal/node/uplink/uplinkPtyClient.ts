/*---------------------------------------------------------------------------------------------
 * uplink-pty client: Communicates with Rust PTY service over Unix socket
 * Wire format: [1 byte tag][4 byte length BE][MessagePack payload]
 *--------------------------------------------------------------------------------------------*/

import * as net from 'net';
import { EventEmitter } from 'events';
import { encode, decode } from '@msgpack/msgpack';

// Message type tags - must match Rust protocol.rs
const MSG_CREATE = 1;
const MSG_INPUT = 2;
const MSG_RESIZE = 3;
const MSG_KILL = 4;
const MSG_CREATED = 10;
const MSG_OK = 11;
const MSG_ERROR = 12;
const MSG_DATA = 20;
const MSG_EXIT = 21;

export interface CreateRequest {
	id: number;
	shell: string;
	args: string[];
	cwd: string;
	env: Record<string, string>;
	cols: number;
	rows: number;
}

export interface CreatedResponse {
	id: number;
	terminal_id: number;
	pid: number;
}

export interface DataEvent {
	terminal_id: number;
	data: Uint8Array;
}

export interface ExitEvent {
	terminal_id: number;
	code: number | null;
}

type PendingRequest = {
	resolve: (value: any) => void;
	reject: (error: Error) => void;
};

export class UplinkPtyClient extends EventEmitter {
	private socket: net.Socket | null = null;
	private buffer: Buffer = Buffer.alloc(0);
	private nextId = 1;
	private pending = new Map<number, PendingRequest>();

	constructor(private socketPath: string) {
		super();
	}

	async connect(): Promise<void> {
		return new Promise((resolve, reject) => {
			this.socket = net.createConnection(this.socketPath, () => {
				resolve();
			});

			this.socket.on('error', (err) => {
				reject(err);
				this.emit('error', err);
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
				break; // Wait for more data
			}

			const payload = this.buffer.subarray(5, 5 + len);
			this.buffer = this.buffer.subarray(5 + len);

			this.handleMessage(tag, payload);
		}
	}

	private handleMessage(tag: number, payload: Buffer): void {
		const msg = decode(payload) as any;
		console.log(`[UplinkPtyClient] handleMessage tag=${tag}`, msg);

		switch (tag) {
			case MSG_CREATED: {
				const pending = this.pending.get(msg.id);
				pending?.resolve(msg);
				this.pending.delete(msg.id);
				break;
			}
			case MSG_OK: {
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
			case MSG_DATA: {
				this.emit('data', msg.terminal_id, Buffer.from(msg.data));
				break;
			}
			case MSG_EXIT: {
				this.emit('exit', msg.terminal_id, msg.code);
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

	async create(opts: Omit<CreateRequest, 'id'>): Promise<CreatedResponse> {
		const id = this.nextId++;
		return this.request<CreatedResponse>(MSG_CREATE, { id, ...opts }, id);
	}

	async input(terminalId: number, data: Buffer): Promise<void> {
		console.log(`[UplinkPtyClient] input terminalId=${terminalId} data.length=${data.length}`);
		const id = this.nextId++;
		await this.request(MSG_INPUT, { id, terminal_id: terminalId, data: Array.from(data) }, id);
	}

	async resize(terminalId: number, cols: number, rows: number): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_RESIZE, { id, terminal_id: terminalId, cols, rows }, id);
	}

	async kill(terminalId: number): Promise<void> {
		const id = this.nextId++;
		await this.request(MSG_KILL, { id, terminal_id: terminalId }, id);
	}

	close(): void {
		this.socket?.destroy();
		this.socket = null;
	}
}
