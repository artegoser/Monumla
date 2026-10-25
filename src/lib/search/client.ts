import type {
	AnalysisView,
	MoveAssessment,
	SearchMessage,
	SearchRequest,
	SearchSettings
} from './types';

export class SearchCancelled extends Error {
	constructor() {
		super('Search cancelled');
		this.name = 'SearchCancelled';
	}
}

/** Position changes cancel between bounded chunks while retaining engine memory. */
export class SearchClient {
	private worker: Worker | null = null;
	private reject: ((reason: Error) => void) | null = null;
	private generation = 0;
	private control: Int32Array | null = null;

	analyze(
		snapshot: unknown,
		settings: SearchSettings,
		onProgress: (analysis: AnalysisView) => void
	): Promise<AnalysisView> {
		return this.request(snapshot, settings, onProgress) as Promise<AnalysisView>;
	}
	review(snapshot: unknown, ply: number): Promise<MoveAssessment> {
		return this.request(
			snapshot,
			{ maxDepth: 3, maxNodes: 10000, timeMs: 6000, lines: 2 },
			() => {},
			ply
		) as Promise<MoveAssessment>;
	}
	private request(
		snapshot: unknown,
		settings: SearchSettings,
		onProgress: (analysis: AnalysisView) => void,
		ply?: number
	): Promise<AnalysisView | MoveAssessment> {
		this.cancel();
		const id = ++this.generation;
		return new Promise((resolve, reject) => {
			try {
				const worker =
					this.worker ??
					new Worker(new URL('./engine.worker.ts', import.meta.url), {
						type: 'module'
					});
				this.worker = worker;
				this.reject = reject;
				const finish = () => {
					if (this.worker === worker) {
						this.reject = null;
						this.control = null;
					}
				};
				worker.onmessage = ({ data }: MessageEvent<SearchMessage>) => {
					if (id !== this.generation || data.id !== id) return;
					if (data.type === 'error') {
						finish();
						reject(new Error(data.message));
					} else if (data.type === 'progress') {
						onProgress(data.analysis);
					} else {
						finish();
						resolve(data.type === 'review' ? data.review : data.analysis);
					}
				};
				worker.onerror = (event) => {
					finish();
					worker.terminate();
					this.worker = null;
					reject(new Error(event.message || 'The analysis worker could not start. Try again.'));
				};
				const shared =
					globalThis.crossOriginIsolated && typeof SharedArrayBuffer !== 'undefined' && ply == null
						? new SharedArrayBuffer(4)
						: undefined;
				this.control = shared ? new Int32Array(shared) : null;
				const request: SearchRequest = { id, snapshot, settings, ply, control: shared };
				worker.postMessage(request);
			} catch (cause) {
				// Cleanup must not settle this promise as cancellation before
				// the actual startup/serialization error reaches the panel.
				this.reject = null;
				this.cancel();
				reject(cause instanceof Error ? cause : new Error(String(cause)));
			}
		});
	}

	dispose() {
		this.cancel();
		this.worker?.terminate();
		this.worker = null;
	}
	cancel() {
		if (this.control) {
			Atomics.store(this.control, 0, 1);
			this.control = null;
		}
		this.generation += 1;
		this.worker?.postMessage({ id: this.generation, cancel: true });
		const reject = this.reject;
		this.reject = null;
		reject?.(new SearchCancelled());
	}
}
