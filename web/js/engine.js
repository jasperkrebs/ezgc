// Engine client: the Rust engine runs as WebAssembly in a Web Worker, so the UI never freezes.
//
// Every request carries an id and gets one answer. For runs only the newest matters (like a game
// dropping stale frames): while the worker is busy, a new run replaces any queued one, and the
// replaced run resolves with `null`. If the engine ever crashes (a bug), the worker is replaced
// by a fresh one: the request that hit it fails with a message, and the others are asked again.

/**
 * @typedef {{ key: string, message: string }} Problem
 * @typedef {{ name: string, stats: any, massG: number | null, cost: number | null, blocked: string | null }} PartMeta
 * @typedef {{ positions: Float32Array, feeds: Float32Array, travel: Uint8Array }} Preview
 * @typedef {{ kind: any, settings: any, printer: any, problems: Problem[], printerProblems: Problem[],
 *   error: string | null, warnings: string[], hints: { key: string, text: string }[], parts: PartMeta[],
 *   preview: Preview[], ms: number }} RunResult
 */

export function createEngine() {
  /** @type {Worker} */
  let worker;
  /** @type {(info: { version: string, loadMs: number }) => void} */
  let onReady = () => {};
  /** @type {(err: Error) => void} */
  let onFail = () => {};
  let started = false;
  const ready = new Promise((resolve, reject) => {
    onReady = resolve;
    onFail = reject;
  });

  let nextId = 1;
  /** Requests the worker hasn't answered yet, by id. @type {Map<number, { msg: object, resolve: (r: any) => void, reject: (e: Error) => void }>} */
  const pending = new Map();

  /** @type {{ project: string, printer: string, resolve: (r: RunResult | null) => void, reject: (e: Error) => void } | null} */
  let queued = null;
  let running = false;

  function start() {
    worker = new Worker(new URL('./engine.worker.js', import.meta.url), { type: 'module' });
    worker.onmessage = onMessage;
    worker.onerror = (e) => {
      if (!started) fail(new Error(e.message || 'The engine could not start.'));
    };
  }

  /** The engine can't start: everything waiting on it fails. */
  function fail(err) {
    onFail(err);
    for (const r of pending.values()) r.reject(err);
    pending.clear();
  }

  /** @param {{ type: string } & Record<string, any>} msg */
  function send(msg) {
    return new Promise((resolve, reject) => {
      const withId = { ...msg, id: nextId++ };
      pending.set(withId.id, { msg: withId, resolve, reject });
      worker.postMessage(withId);
    });
  }

  /** @param {MessageEvent} e */
  function onMessage(e) {
    const msg = e.data;
    if (msg.type === 'ready') {
      started = true;
      onReady({ version: msg.version, loadMs: msg.loadMs });
      return;
    }
    if (msg.type === 'fatal') {
      fail(new Error(msg.error));
      return;
    }
    const request = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.type === 'crashed') {
      worker.terminate();
      start();
      // Whatever the old worker never answered is asked again.
      for (const r of pending.values()) worker.postMessage(r.msg);
    }
    if (!request) return;
    if (msg.type === 'result') request.resolve(msg.result);
    else request.reject(new Error(msg.type === 'crashed' ? `The engine hit an internal error and restarted (${msg.error}).` : msg.error));
  }

  function pump() {
    if (running || !queued) return;
    const job = queued;
    queued = null;
    running = true;
    send({ type: 'run', project: job.project, printer: job.printer })
      .then(job.resolve, job.reject)
      .finally(() => {
        running = false;
        pump();
      });
  }

  start();

  return {
    ready,

    /**
     * Checks a project and a printer profile and builds the toolpaths. Resolves with `null` when a
     * newer run replaced this one before it started.
     * @param {string} project the project file's text
     * @param {string} printer the printer profile's settings (JSON)
     * @returns {Promise<RunResult | null>}
     */
    run(project, printer) {
      return new Promise((resolve, reject) => {
        if (queued) queued.resolve(null);
        queued = { project, printer, resolve, reject };
        pump();
      });
    },

    /** Every kind of project. @returns {Promise<{ id: string, name: string, description: string, version: number }[]>} */
    kinds: () => send({ type: 'kinds' }),
    /** A kind's settings window and defaults. @param {string} kind */
    schema: (kind) => send({ type: 'schema', kind }),
    /** The printer's settings window and the machines a profile can be based on. */
    printerSchema: () => send({ type: 'printerSchema' }),
    /** A new printer profile's settings, based on a machine. @param {string} model */
    newPrinter: (model) => send({ type: 'newPrinter', model }),
    /** A new project of a kind, every setting at its default. @param {string} kind */
    newProject: (kind) => send({ type: 'newProject', kind }),
    /**
     * The G-code of one part for a printer. Fails with a readable message when it can't be written.
     * @param {string} project @param {string} printer @param {number} part @returns {Promise<string>}
     */
    gcode: (project, printer, part = 0) => send({ type: 'gcode', project, printer, part }),
  };
}
