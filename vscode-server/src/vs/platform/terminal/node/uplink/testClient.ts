/*---------------------------------------------------------------------------------------------
 * Test script for uplink-pty Node client
 * Run: npx ts-node src/vs/platform/terminal/node/uplink/testClient.ts
 *--------------------------------------------------------------------------------------------*/

import { UplinkPtyClient } from './uplinkPtyClient.js';
import * as readline from 'readline';

async function main() {
	const client = new UplinkPtyClient('/tmp/uplink-pty.sock');

	client.on('data', (terminalId: number, data: Buffer) => {
		process.stdout.write(data);
	});

	client.on('exit', (terminalId: number, code: number | null) => {
		console.log(`\nTerminal ${terminalId} exited with code ${code}`);
		process.exit(0);
	});

	client.on('error', (err: Error) => {
		console.error('Client error:', err);
	});

	try {
		await client.connect();
		console.log('Connected to uplink-pty');

		const result = await client.create({
			shell: process.env.SHELL || '/bin/bash',
			args: ['-l'],
			cwd: process.env.HOME || '/',
			env: {},
			cols: 80,
			rows: 24
		});

		console.log(`Created terminal ${result.terminal_id} with pid ${result.pid}`);

		// Set up stdin for raw input
		if (process.stdin.isTTY) {
			process.stdin.setRawMode(true);
		}
		process.stdin.resume();

		process.stdin.on('data', async (data) => {
			try {
				await client.input(result.terminal_id, data);
			} catch (err) {
				console.error('Input error:', err);
			}
		});

		// Handle Ctrl+C
		process.on('SIGINT', async () => {
			await client.kill(result.terminal_id);
			client.close();
			process.exit(0);
		});

	} catch (err) {
		console.error('Failed to connect:', err);
		process.exit(1);
	}
}

main();
