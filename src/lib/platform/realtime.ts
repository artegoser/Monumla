import { writable } from 'svelte/store';
import type { Lobby, Personal } from './types';

export const connection = writable<'connecting' | 'online' | 'offline'>('connecting');
export const liveLobby = writable<Lobby | null>(null);
export const livePersonal = writable<Personal | null>(null);
type Listener = (data: unknown) => void;
const listeners = new Map<string, Set<Listener>>();
const pending = new Map<
	string,
	{
		resolve: (value: unknown) => void;
		reject: (error: Error) => void;
		timer: ReturnType<typeof setTimeout>;
	}
>();
let socket: WebSocket | null = null;
let reconnect: ReturnType<typeof setTimeout> | null = null;
let sequence = 0;
let attempts = 0;
let identity: string | null | undefined;
let running = false;

export function onRealtime<T>(type: string, listener: (data: T) => void) {
	const set = listeners.get(type) ?? new Set<Listener>();
	set.add(listener as Listener);
	listeners.set(type, set);
	return () => {
		set.delete(listener as Listener);
	};
}
function emit(type: string, data: unknown) {
	for (const listener of listeners.get(type) ?? []) listener(data);
}

export function connect(userId: string | null) {
	if (running && identity === userId) return;
	disconnect();
	identity = userId;
	running = true;
	function open() {
		if (!running) return;
		connection.set('connecting');
		const ws = new WebSocket(
			`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/ws`
		);
		socket = ws;
		ws.onopen = () => {
			attempts = 0;
			connection.set('online');
			emit('connected', null);
		};
		ws.onmessage = (event) => {
			const message = JSON.parse(event.data);
			if (message.id) {
				const request = pending.get(message.id);
				if (!request) return;
				clearTimeout(request.timer);
				pending.delete(message.id);
				if (message.ok) request.resolve(message.data);
				else request.reject(new Error(message.message));
			} else {
				if (message.type === 'lobby') liveLobby.set(message.data);
				if (message.type === 'personal') livePersonal.set(message.data);
				emit(message.type, message.data ?? message.message);
			}
		};
		ws.onclose = () => {
			if (socket !== ws) return;
			connection.set('offline');
			for (const request of pending.values()) {
				clearTimeout(request.timer);
				request.reject(new Error('Connection lost. Reconnecting…'));
			}
			pending.clear();
			if (running) reconnect = setTimeout(open, Math.min(10_000, 500 * 2 ** attempts++));
		};
		ws.onerror = () => ws.close();
	}
	open();
}

export function disconnect() {
	running = false;
	if (reconnect) clearTimeout(reconnect);
	const old = socket;
	socket = null;
	old?.close();
	for (const request of pending.values()) {
		clearTimeout(request.timer);
		request.reject(new Error('Connection closed.'));
	}
	pending.clear();
	livePersonal.set(null);
	liveLobby.set(null);
}

export function request<T = unknown>(type: string, data: Record<string, unknown> = {}): Promise<T> {
	if (socket?.readyState !== WebSocket.OPEN)
		return Promise.reject(new Error('Connecting to Monumla. Please try again in a moment.'));
	const id = String(++sequence);
	return new Promise<T>((resolve, reject) => {
		const timer = setTimeout(() => {
			pending.delete(id);
			reject(new Error('The request timed out. Please reconnect and try again.'));
		}, 15_000);
		pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
		socket!.send(JSON.stringify({ ...data, type, id }));
	});
}
