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
					if (this.worker === worker) this.reject = null;
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
				const request: SearchRequest = { id, snapshot, settings, ply };
				worker.postMessage(request);
			} catch (cause) {
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
		this.generation += 1;
		this.worker?.postMessage({ id: this.generation, cancel: true });
		const reject = this.reject;
		this.reject = null;
		reject?.(new SearchCancelled());
	}
}
