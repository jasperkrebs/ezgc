// The project library: every project on this device, saved as you go, plus project files in and
// out. A project is EasyGCode 2's project file (see crates/engine/src/project.rs) with three
// fields of bookkeeping the engine ignores: `id`, `created` and `modified`.
//
// Storage is the browser's localStorage, as in EasyGCode: one entry per project, an index, and the
// previous version of each project as a backup in case an entry is ever damaged. iOS can clear a
// web app's storage after weeks unused, so "Export all" (one file with every project) is the
// durable copy, as EasyGCode's settings file was.

const PREFIX = 'easygcode2.';
const INDEX = `${PREFIX}index`;
const CURRENT = `${PREFIX}current`;
const projectKey = (id) => `${PREFIX}p.${id}`;

/** @typedef {{ id: string, name: string, kind: string, modified: string }} Entry */

/** A new project id: short, random, safe in a storage key. */
export function newId() {
  const bytes = new Uint8Array(8);
  globalThis.crypto.getRandomValues(bytes);
  return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
}

/** The project as a file to save or share: the project itself, without this device's bookkeeping. */
export function toFile(project) {
  const { id, created, modified, ...file } = project;
  return JSON.stringify(file, null, 2) + '\n';
}

/** Every project of this device as one file (a backup). */
export function bundle(projects) {
  return JSON.stringify({ app: 'easygcode', format: 2, projects: projects.map((p) => JSON.parse(toFile(p))) }, null, 2) + '\n';
}

/**
 * Reads a file: one project, or a backup of several. The engine checks each project when it's
 * opened; this only finds them.
 * @param {string} text
 * @returns {{ projects: any[] } | { error: string }}
 */
export function parseFile(text) {
  let v;
  try {
    v = JSON.parse(text);
  } catch {
    return { error: "That file isn't a project (it isn't JSON)." };
  }
  if (!v || typeof v !== 'object') return { error: "That file isn't a project." };
  if (Array.isArray(v.projects)) {
    const projects = v.projects.filter((p) => p && typeof p === 'object' && typeof p.kind === 'string');
    return projects.length ? { projects } : { error: 'That backup has no projects in it.' };
  }
  if (v.version === 1 && v.settings && v.format === undefined) {
    return { error: "This is an EasyGCode 1 settings file (the old app's tabs). Opening those isn't built yet." };
  }
  if (typeof v.kind !== 'string') return { error: "That file isn't an EasyGCode 2 project (it has no kind)." };
  return { projects: [v] };
}

/**
 * @param {Storage} [storage]
 */
export function createLibrary(storage = globalThis.localStorage) {
  /** Whether the last write reached the device's storage. */
  let ok = true;

  function read(key) {
    try {
      return storage.getItem(key);
    } catch {
      ok = false;
      return null;
    }
  }

  function write(key, value) {
    try {
      storage.setItem(key, value);
      ok = true;
    } catch {
      ok = false; // full, or private browsing
    }
    return ok;
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

  /** @param {Entry[]} entries */
  function writeIndex(entries) {
    write(INDEX, JSON.stringify(entries));
  }

  const lib = {
    /** Whether storage works (false: private browsing or full; projects live only in this tab). */
    get ok() {
      return ok;
    },

    /** The projects on this device, the last changed first. @returns {Entry[]} */
    list() {
      return index().sort((a, b) => String(b.modified).localeCompare(String(a.modified)));
    },

    /** A project by id, or null. A damaged entry falls back to its previous version. */
    load(id) {
      for (const key of [projectKey(id), `${projectKey(id)}.prev`]) {
        try {
          const p = JSON.parse(read(key) || 'null');
          if (p && typeof p === 'object' && typeof p.kind === 'string') return { ...p, id };
        } catch {
          // Damaged: try the backup.
        }
      }
      return null;
    },

    /**
     * Saves a project (a new one gets an id), and returns it as saved.
     * @param {any} project
     */
    save(project) {
      const now = new Date().toISOString();
      const p = { ...project, id: project.id || newId(), created: project.created || now, modified: now };
      const key = projectKey(p.id);
      const text = JSON.stringify(p);
      const prev = read(key);
      if (prev && prev !== text) write(`${key}.prev`, prev);
      write(key, text);
      const entries = index().filter((e) => e.id !== p.id);
      entries.push({ id: p.id, name: String(p.name || ''), kind: p.kind, modified: now });
      writeIndex(entries);
      return p;
    },

    /** Deletes a project from this device. */
    remove(id) {
      try {
        storage.removeItem(projectKey(id));
        storage.removeItem(`${projectKey(id)}.prev`);
      } catch {
        ok = false;
      }
      writeIndex(index().filter((e) => e.id !== id));
      if (lib.current() === id) write(CURRENT, '');
    },

    /** A copy of a project, saved under a new id. */
    duplicate(id) {
      const p = lib.load(id);
      if (!p) return null;
      const { id: _, created, modified, ...rest } = p;
      return lib.save({ ...rest, name: `${p.name || 'Project'} (copy)` });
    },

    /** The project open last time. */
    current() {
      return read(CURRENT) || null;
    },

    setCurrent(id) {
      write(CURRENT, id || '');
    },
  };
  return lib;
}
