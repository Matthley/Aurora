import { afterEach, describe, expect, it, vi } from 'vitest';
import { isRemoteHost } from './phone';
import { collect, deviceKind, hydrate, keepSynced } from './syncSettings';

function memory(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    get length() {
      return Object.keys(data).length;
    },
    key: (i: number) => Object.keys(data)[i] ?? null,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => {
      data[k] = v;
    },
  };
}

const reply = (body: unknown, ok = true) => Promise.resolve({ ok, json: () => Promise.resolve(body) } as Response);

describe('syncSettings', () => {
  it('tells the main PC from a remote client by how it is connected, not by width', () => {
    expect(isRemoteHost({ location: { hostname: '127.0.0.1' } })).toBe(false);
    expect(isRemoteHost({ location: { hostname: 'localhost' } })).toBe(false);
    expect(isRemoteHost({ location: { hostname: '192.168.1.20' }, __TAURI_INTERNALS__: {} } as never)).toBe(false);
    expect(isRemoteHost({ location: { hostname: '192.168.1.20' } })).toBe(true);
    expect(deviceKind(true)).toBe('mobile');
    expect(deviceKind(false)).toBe('desktop');
  });

  it('collects only the Aurora keys', () => {
    expect(collect(memory({ 'aurora-prefs': '1', altro: '2' }))).toEqual({ 'aurora-prefs': '1' });
  });

  it('brings the server preferences into the browser, the server wins', async () => {
    const store = memory({ 'aurora-prefs': 'vecchio' });
    const fetcher = vi.fn(() => reply({ 'aurora-prefs': 'dal-server', 'aurora-layout': '[]', estraneo: 'x' })) as unknown as typeof fetch;
    await hydrate(store, 'desktop', fetcher);
    expect(store.data).toEqual({ 'aurora-prefs': 'dal-server', 'aurora-layout': '[]' });
  });

  it('the main PC sends the browser preferences when the server has none yet', async () => {
    const store = memory({ 'aurora-prefs': 'mie' });
    const calls: [string, RequestInit | undefined][] = [];
    const fetcher = vi.fn((url: string, init?: RequestInit) => {
      calls.push([url, init]);
      return reply({});
    }) as unknown as typeof fetch;
    await hydrate(store, 'desktop', fetcher);
    const put = calls.find(([, init]) => init?.method === 'PUT');
    expect(put?.[0]).toBe('/api/v1/ui-settings/desktop');
    expect(JSON.parse(String(put?.[1]?.body))).toEqual({ 'aurora-prefs': 'mie' });
  });

  it('a remote client never sends its preferences while loading', async () => {
    const store = memory({ 'aurora-prefs': 'mie', 'aurora-layout': '[]' });
    const fetcher = vi.fn(() => reply({})) as unknown as typeof fetch;
    await hydrate(store, 'mobile', fetcher);
    const methods = (fetcher as unknown as ReturnType<typeof vi.fn>).mock.calls.map((c) => (c[1] as RequestInit | undefined)?.method ?? 'GET');
    expect(methods.every((m) => m === 'GET')).toBe(true);
  });

  it('keeps what the browser has when the server cannot be reached', async () => {
    const store = memory({ 'aurora-prefs': 'mie' });
    await hydrate(store, 'desktop', (() => Promise.reject(new Error('rete'))) as unknown as typeof fetch);
    await hydrate(store, 'desktop', (() => reply({}, false)) as unknown as typeof fetch);
    expect(store.data).toEqual({ 'aurora-prefs': 'mie' });
  });

  describe('keepSynced', () => {
    let stop: (() => void) | undefined;
    afterEach(() => {
      stop?.();
      stop = undefined;
      localStorage.clear();
      vi.useRealTimers();
    });

    function server(initial: Record<string, string>) {
      const puts: [string, Record<string, string>][] = [];
      const fetcher = vi.fn((url: string, init?: RequestInit) => {
        if (init?.method === 'PUT') {
          puts.push([url, JSON.parse(String(init.body)) as Record<string, string>]);
          return reply({});
        }
        return reply(initial);
      }) as unknown as typeof fetch;
      return { puts, fetcher };
    }

    it('sends nothing before the first click or key', async () => {
      vi.useFakeTimers();
      const target = new EventTarget();
      const { puts, fetcher } = server({});
      stop = keepSynced('desktop', fetcher, { storage: localStorage, target });
      localStorage.setItem('aurora-prefs', 'scritto-da-solo');
      await vi.advanceTimersByTimeAsync(1000);
      expect(fetcher).not.toHaveBeenCalled();
      expect(puts).toEqual([]);
    });

    it('after a gesture sends only the changed keys, merged with what the server has', async () => {
      vi.useFakeTimers();
      const target = new EventTarget();
      localStorage.setItem('aurora-altro', 'locale-non-cambiato');
      const { puts, fetcher } = server({ 'aurora-prefs': 'vecchio', 'aurora-tema': 'scuro' });
      stop = keepSynced('desktop', fetcher, { storage: localStorage, target });
      target.dispatchEvent(new Event('pointerdown'));
      localStorage.setItem('aurora-prefs', 'nuovo');
      await vi.advanceTimersByTimeAsync(1000);
      expect(puts).toEqual([['/api/v1/ui-settings/desktop', { 'aurora-prefs': 'nuovo', 'aurora-tema': 'scuro' }]]);
    });

    it('a remote client saves only its own layout, never the PC settings', async () => {
      vi.useFakeTimers();
      const target = new EventTarget();
      const { puts, fetcher } = server({});
      stop = keepSynced('mobile', fetcher, { storage: localStorage, target });
      target.dispatchEvent(new Event('keydown'));
      localStorage.setItem('aurora-filters', '{}');
      localStorage.setItem('aurora-layout', '[1]');
      await vi.advanceTimersByTimeAsync(1000);
      expect(puts).toEqual([['/api/v1/ui-settings/mobile', { 'aurora-layout': '[1]' }]]);
      expect(puts.some(([url]) => url.endsWith('/desktop'))).toBe(false);
    });
  });
});
