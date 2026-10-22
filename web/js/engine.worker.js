// Web Worker that hosts the Rust engine (WebAssembly). Generated glue lives in ../pkg/
// (scripts/build-web.sh). Preview arrays go back to the page as transferables: moved, not copied.
// Every request carries an id, and its answer (result, error or crash) carries the same id.

import init, * as engine from '../pkg/ezgc_wasm.js';

const started = performance.now();
const loading = init()
  .then(() => {
    postMessage({ type: 'ready', version: engine.engineVersion(), loadMs: performance.now() - started });
  })
  .catch((err) => {
    postMessage({ type: 'fatal', error: String((err && err.message) || err) });
    throw err;
  });

/** Answers one request: the result, and the buffers to move rather than copy. */
function answer(msg) {
  switch (msg.type) {
    case 'kinds':
      return { result: JSON.parse(engine.kinds()), transfer: [] };
    case 'schema':
      return { result: JSON.parse(engine.schema(msg.kind)), transfer: [] };
    case 'printerSchema':
      return { result: JSON.parse(engine.printerSchema()), transfer: [] };
    case 'newProject':
      return { result: JSON.parse(engine.newProject(msg.kind)), transfer: [] };
    case 'newPrinter':
      return { result: JSON.parse(engine.newPrinter(msg.model)), transfer: [] };
    case 'gcode':
      return { result: engine.gcode(msg.project, msg.printer, msg.part), transfer: [] };
    case 'run': {
      const t0 = performance.now();
      const r = engine.run(msg.project, msg.printer);
      const ms = performance.now() - t0;
      const parts = Array.from(r.parts);
      const transfer = parts.flatMap((p) => [p.positions.buffer, p.feeds.buffer, p.travel.buffer]);
      return { result: { ...JSON.parse(r.meta), preview: parts, ms }, transfer };
    }
    default:
      throw new Error(`unknown request: ${msg.type}`);
  }
}

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    await loading;
  } catch {
    return; // already reported as fatal
  }
  try {
    const { result, transfer } = answer(msg);
    postMessage({ type: 'result', id: msg.id, result }, transfer);
  } catch (err) {
    // A trap inside the WebAssembly is an engine bug; the instance can't be trusted after it,
    // so the page starts a fresh worker. Anything else is an ordinary error message.
    const crashed = err instanceof WebAssembly.RuntimeError;
    postMessage({ type: crashed ? 'crashed' : 'error', id: msg.id, error: String((err && err.message) || err) });
  }
};
