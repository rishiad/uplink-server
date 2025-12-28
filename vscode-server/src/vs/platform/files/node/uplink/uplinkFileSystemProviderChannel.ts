/*---------------------------------------------------------------------------------------------
 * UplinkFileSystemProviderChannel: IPC channel for UplinkFileSystemProvider
 *--------------------------------------------------------------------------------------------*/

import { Emitter } from '../../../../base/common/event.js';
import { URI, UriComponents } from '../../../../base/common/uri.js';
import { IURITransformer } from '../../../../base/common/uriIpc.js';
import { IFileChange, IWatchOptions } from '../../common/files.js';
import { ILogService } from '../../../log/common/log.js';
import { createURITransformer } from '../../../../base/common/uriTransformer.js';
import { RemoteAgentConnectionContext } from '../../../remote/common/remoteAgentEnvironment.js';
import { AbstractDiskFileSystemProviderChannel, ISessionFileWatcher } from '../diskFileSystemProviderServer.js';
import { UplinkFileSystemProvider } from './uplinkFileSystemProvider.js';
import { Disposable, IDisposable, toDisposable } from '../../../../base/common/lifecycle.js';

export class UplinkFileSystemProviderChannel extends AbstractDiskFileSystemProviderChannel<RemoteAgentConnectionContext> {

	private readonly uriTransformerCache = new Map<string, IURITransformer>();

	constructor(
		logService: ILogService,
	) {
		// Cast to any because AbstractDiskFileSystemProviderChannel expects DiskFileSystemProvider
		// but UplinkFileSystemProvider implements the same interface
		super(new UplinkFileSystemProvider(logService) as any, logService);

		this._register(this.provider);
	}

	protected override getUriTransformer(ctx: RemoteAgentConnectionContext): IURITransformer {
		let transformer = this.uriTransformerCache.get(ctx.remoteAuthority);
		if (!transformer) {
			transformer = createURITransformer(ctx.remoteAuthority);
			this.uriTransformerCache.set(ctx.remoteAuthority, transformer);
		}
		return transformer;
	}

	protected override transformIncoming(uriTransformer: IURITransformer, _resource: UriComponents, supportVSCodeResource = false): URI {
		if (supportVSCodeResource && _resource.path === '/vscode-resource' && _resource.query) {
			const requestResourcePath = JSON.parse(_resource.query).requestResourcePath;
			return URI.from({ scheme: 'file', path: requestResourcePath });
		}
		return URI.revive(uriTransformer.transformIncoming(_resource));
	}

	protected createSessionFileWatcher(uriTransformer: IURITransformer, emitter: Emitter<IFileChange[] | string>): ISessionFileWatcher {
		return new UplinkSessionFileWatcher(uriTransformer, emitter, this.provider as unknown as UplinkFileSystemProvider);
	}
}

class UplinkSessionFileWatcher extends Disposable implements ISessionFileWatcher {

	private readonly watcherRequests = new Map<number, IDisposable>();

	constructor(
		private readonly uriTransformer: IURITransformer,
		private readonly sessionEmitter: Emitter<IFileChange[] | string>,
		private readonly provider: UplinkFileSystemProvider
	) {
		super();

		this._register(this.provider.onDidChangeFile(events => {
			this.sessionEmitter.fire(
				events.map(e => ({
					resource: this.uriTransformer.transformOutgoingURI(e.resource),
					type: e.type,
					cId: (e as any).cId
				}))
			);
		}));

		this._register(this.provider.onDidWatchError(error => {
			this.sessionEmitter.fire(error);
		}));
	}

	watch(req: number, resource: URI, opts: IWatchOptions): IDisposable {
		this.watcherRequests.set(req, this.provider.watch(resource, opts));

		return toDisposable(() => {
			const disposable = this.watcherRequests.get(req);
			disposable?.dispose();
			this.watcherRequests.delete(req);
		});
	}

	override dispose(): void {
		for (const [, disposable] of this.watcherRequests) {
			disposable.dispose();
		}
		this.watcherRequests.clear();
		super.dispose();
	}
}
