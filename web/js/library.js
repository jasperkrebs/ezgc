// The library on this device: projects and printer profiles, saved as you go, plus files in and
// out. A project is ezgc's project file (see crates/engine/src/project.rs); a printer profile is
// `{ type: 'printer', name, settings }`. Both carry three fields of bookkeeping the engine never
// sees: `id`, `created` and `modified`.
//
// Storage is the browser's localStorage: one entry per item, an index per collection, and the
// previous version of each item as a backup in case an entry is ever damaged. iOS can clear a web
// app's storage after weeks unused, so "Export all" (one file with everything) is the durable copy.

const PREFIX = 'ezgc.';

/** @typedef {{ id: string, name: string, kind: string, modified: string }} Entry */

/** A new id: short, random, safe in a storage key. */
export function newId() {
  const bytes = new Uint8Array(8);
  globalThis.crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** An item without this device's bookkeeping. */
function strip(item) {
  const { id, created, modified, ...rest } = item;
  return rest;
}

/** A project or printer as a file to save or share. */
export function toFile(item) {
  return JSON.stringify({ app: 'ezgc', format: 1, ...strip(item) }, null, 2) + '\n';
}

/** Everything on this device as one file (a backup). */
export function bundle(projects, printers) {
  return (
    JSON.stringify(
      { app: 'ezgc', format: 1, projects: projects.map(strip), printers: printers.map(strip) },
      null,
      2,
    ) + '\n'
  );
}

const isPrinter = (v) => v && typeof v === 'object' && v.type === 'printer' && v.settings && typeof v.settings === 'object';
const isProject = (v) => v && typeof v === 'object' && typeof v.kind === 'string';

/**
 * Reads a file: a project, a printer profile, or a backup of several. The engine checks each one
 * when it's used; this only finds them.
 * @param {string} text
 * @returns {{ projects: any[], printers: any[] } | { error: string }}
 */
export function parseFile(text) {
  let v;
  try {
    v = JSON.parse(text);
  } catch {
    return { error: "That file isn't an ezgc file (it isn't JSON)." };
  }
  if (!v || typeof v !== 'object' || Array.isArray(v)) return { error: "That file isn't an ezgc file." };
  if (v.app !== undefined && v.app !== 'ezgc') return { error: `That file is from another app (${v.app}).` };
  if (Array.isArray(v.projects) || Array.isArray(v.printers)) {
    const projects = (v.projects || []).filter(isProject);
    const printers = (v.printers || []).filter(isPrinter);
    return projects.length || printers.length ? { projects, printers } : { error: 'That backup is empty.' };
  }
  if (isPrinter(v)) return { projects: [], printers: [v] };
  if (isProject(v)) return { projects: [v], printers: [] };
  return { error: "That file isn't an ezgc project or printer." };
}

/**
 * One collection of the library (`projects` or `printers`).
 * @param {Storage} storage
 * @param {string} name
 * @param {(item: any) => boolean} valid
 * @param {{ ok: boolean }} health shared: whether the last write reached the device's storage
 */
function createStore(storage, name, valid, health) {
  const INDEX = `${PREFIX}${name}`;
  const CURRENT = `${PREFIX}${name}.current`;
  const key = (id) => `${PREFIX}${name}.${id}`;

  function read(k) {
    try {
      return storage.getItem(k);
    } catch {
      health.ok = false;
      return null;
    }
  }

  function write(k, value) {
    try {
      storage.setItem(k, value);
      health.ok = true;
    } catch {
      health.ok = false; // full, or private browsing
    }
  }

  /** @returns {Entry[]} */
  function index() {
    try {
      const v = JSON.parse(read(INDEX) || '[]');
      return Array.isArray(v) ? v.filter((e) => e && typeof e.id === 'string') : [];
    } catch {
      return [];
    }
  }

  const store = {
    /** Newest first. @returns {Entry[]} */
    list() {
      return index().sort((a, b) => String(b.modified).localeCompare(String(a.modified)));
    },

    /** An item by id, or null. A damaged entry falls back to its previous version. */
    load(id) {
      for (const k of [key(id), `${key(id)}.prev`]) {
        try {
          const item = JSON.parse(read(k) || 'null');
          if (valid(item)) return { ...item, id };
        } catch {
          // Damaged: try the backup.
        }
      }
      return null;
    },

    /** Every item, newest first. */
    all() {
      return store.list().map((e) => store.load(e.id)).filter(Boolean);
    },

    /** Saves an item (a new one gets an id) and returns it as saved. */
    save(item) {
      const now = new Date().toISOString();
      const saved = { ...item, id: item.id || newId(), created: item.created || now, modified: now };
      const k = key(saved.id);
      const text = JSON.stringify(saved);
      const prev = read(k);
      if (prev && prev !== text) write(`${k}.prev`, prev);
      write(k, text);
      const entries = index().filter((e) => e.id !== saved.id);
      entries.push({ id: saved.id, name: String(saved.name || ''), kind: saved.kind || saved.settings?.model || '', modified: now });
      write(INDEX, JSON.stringify(entries));
      return saved;
    },

    remove(id) {
      try {
        storage.removeItem(key(id));
        storage.removeItem(`${key(id)}.prev`);
      } catch {
        health.ok = false;
      }
      write(INDEX, JSON.stringify(index().filter((e) => e.id !== id)));
      if (store.current() === id) write(CURRENT, '');
    },

    /** A copy, saved under a new id. */
    duplicate(id) {
      const item = store.load(id);
      return item ? store.save({ ...strip(item), name: `${item.name || 'Untitled'} (copy)` }) : null;
    },

    /** The one in use. */
    current() {
      return read(CURRENT) || null;
    },

    setCurrent(id) {
      write(CURRENT, id || '');
    },
  };
  return store;
}

/** @param {Storage} [storage] */
export function createLibrary(storage = globalThis.localStorage) {
  const health = { ok: true };
  return {
    projects: createStore(storage, 'projects', isProject, health),
    printers: createStore(storage, 'printers', isPrinter, health),
    /** Whether storage works (false: private browsing or full; items live only in this tab). */
    get ok() {
      return health.ok;
    },
  };
}
