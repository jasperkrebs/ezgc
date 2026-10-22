// ezgc: a project (a kind + its settings, as JSON) and a printer profile → the Rust engine
// (WebAssembly, in a worker) → toolpaths in the 3D preview, a print estimate, and G-code to save.
//
// There are no tabs per project: a project names its kind, and its settings window is drawn from
// the kind's schema. Projects and printer profiles live on the device (the Projects sheet and the
// Printer tab), saved as you go, and travel as JSON files. A project doesn't hold a printer: it
// prints on whichever printer is chosen here, so a project sent to someone prints on theirs. Every
// change runs the engine again, live; only the newest run counts.

import { createEngine } from './engine.js';
import { createViewport } from './viewport.js';
import { createForm, set } from './form.js';
import { bundle, createLibrary, parseFile, toFile } from './library.js';
import { $, copyText, download, el, fmt, slug } from './dom.js';

const TABS = ['settings', 'printer', 'gcode'];

const engine = createEngine();
const viewport = createViewport(/** @type {HTMLCanvasElement} */ ($('view')));
const library = createLibrary();

const state = {
  /** The open project: its file, plus the library's id/created/modified. @type {any} */
  project: null,
  /** The printer in use: `{ type: 'printer', name, settings }` plus bookkeeping. @type {any} */
  printer: null,
  /** @type {{ id: string, name: string, description: string, version: number }[]} */
  kinds: [],
  /** Ready-made projects. @type {{ id: string, kind: string, name: string, description: string }[]} */
  templates: [],
  /** Settings windows by kind: `{ info, groups, defaults }`. @type {Map<string, any>} */
  schemas: new Map(),
  /** `{ groups, machines }`. @type {any} */
  printerSchema: null,
  /** @type {ReturnType<typeof createForm> | null} */
  settingsForm: null,
  settingsKind: '',
  /** @type {ReturnType<typeof createForm> | null} */
  printerForm: null,
  /** @type {import('./engine.js').RunResult | null} */
  last: null,
  refit: true,
};

const dialog = () => /** @type {HTMLDialogElement} */ ($('projects'));

// ---------------------------------------------------------------------------------------------
// Running
// ---------------------------------------------------------------------------------------------

/** The project as the engine reads it: the file, bookkeeping left out. */
const projectText = () => toFile(state.project);
const printerText = () => JSON.stringify(state.printer.settings);

/** What a run depends on (names and notes don't change the toolpath). */
const runKey = () => JSON.stringify([state.project.kind, state.project.settings, state.printer.settings]);

async function run() {
  if (!state.project || !state.printer) return;
  const key = runKey();
  $('status').setAttribute('aria-busy', 'true');
  let r;
  try {
    r = await engine.run(projectText(), printerText());
  } catch (err) {
    setStatus('Engine error', 'bad');
    renderNotices([{ kind: 'bad', text: String(err.message || err) }]);
    $('status').setAttribute('aria-busy', 'false');
    return;
  }
  if (!r) return; // a newer run replaced this one
  state.last = r;
  const current = runKey() === key;
  // The engine fills in defaults: keep the complete settings, unless they changed meanwhile.
  if (current && !r.error) {
    state.project.settings = r.settings;
    state.printer.settings = r.printer;
  }
  // A result for older values must not put them back in the fields: show the current ones.
  state.settingsForm?.update(current ? r.settings : state.project.settings, r.problems, r.hints);
  state.printerForm?.update(current ? r.printer : state.printer.settings, r.printerProblems, []);

  const bed = state.printer.settings.bed;
  if (bed) viewport.setBed([bed.width, bed.depth]);
  const shown = viewport.show(r.preview || [], { refit: state.refit });
  state.refit = false;
  renderLegend(shown.feed);

  const notices = [];
  if (r.error) notices.push({ kind: 'bad', text: r.error });
  if (r.problems.length) {
    const n = r.problems.length;
    notices.push({ kind: 'bad', text: `Fix ${n === 1 ? 'one setting' : `${n} settings`} (marked in red) to see the toolpath.` });
  }
  if (r.printerProblems.length) notices.push({ kind: 'bad', text: 'Fix the printer settings marked in red (Printer tab).' });
  for (const w of r.warnings) notices.push({ kind: 'warn', text: w });
  renderNotices(notices);
  $('printerNotices').replaceChildren(
    ...(r.printerProblems.length ? [el('p', { className: 'notice bad', text: 'Fix the printer settings marked in red.' })] : []),
  );
  renderParts(r);

  const fix = r.problems.length + r.printerProblems.length;
  const message = $('viewMessage');
  message.hidden = Boolean(r.preview?.length);
  message.textContent = r.error || (fix ? 'Fix the settings marked in red.' : 'Nothing to show.');
  if (r.error) setStatus('Error', 'bad');
  else if (fix) setStatus('Fix settings', 'bad');
  else if (r.warnings.length) setStatus(r.warnings.length === 1 ? '1 warning' : `${r.warnings.length} warnings`, 'warn');
  else setStatus(`Ready · ${Math.round(r.ms)} ms`, 'ok');
  if (current) $('status').setAttribute('aria-busy', 'false');
}

let saveTimer = 0;
/** Saves soon (a burst of typing is one save). */
function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, 400);
}

function saveNow() {
  clearTimeout(saveTimer);
  if (state.project) state.project = library.projects.save(state.project);
  if (state.printer) state.printer = library.printers.save(state.printer);
  renderHeader();
}

function change(update) {
  update();
  run();
  saveSoon();
}

/** @param {string} key @param {any} value */
function onSettingChange(key, value) {
  change(() => (state.project.settings = set(state.project.settings, key, value)));
}

/** @param {string} key @param {any} value */
function onPrinterChange(key, value) {
  change(() => (state.printer.settings = set(state.printer.settings, key, value)));
}

// ---------------------------------------------------------------------------------------------
// Projects
// ---------------------------------------------------------------------------------------------

/** @param {any} project @param {{ save?: boolean }} [how] */
async function openProject(project, how = {}) {
  if (state.project) saveNow();
  state.project = how.save ? library.projects.save(project) : project;
  library.projects.setCurrent(state.project.id);
  state.refit = true;
  await ensureSettingsForm(state.project.kind);
  $('projectName').value = state.project.name || '';
  $('projectNotes').value = state.project.notes || '';
  renderHeader();
  await run();
}

/** The kind's settings window (built once per kind). */
async function ensureSettingsForm(kind) {
  if (state.settingsKind === kind && state.settingsForm) return;
  let schema = state.schemas.get(kind);
  if (!schema) {
    try {
      schema = await engine.schema(kind);
      state.schemas.set(kind, schema);
    } catch {
      // An unknown kind: the run explains it.
      $('settingsForm').replaceChildren();
      state.settingsForm = null;
      state.settingsKind = '';
      return;
    }
  }
  state.settingsForm = createForm($('settingsForm'), schema.groups, onSettingChange, { remember: `ezgc.open.${kind}` });
  state.settingsKind = kind;
}

async function newProject(kind) {
  await openProject(await engine.newProject(kind), { save: true });
}

/** Opens the last project, or any, or a new one. */
async function openSomething() {
  const id = library.projects.current();
  const p = (id && library.projects.load(id)) || library.projects.all()[0];
  if (p) await openProject(p);
  else await newProject(state.kinds[0]?.id || 'spiral');
}

// ---------------------------------------------------------------------------------------------
// Printers
// ---------------------------------------------------------------------------------------------

function machine(id) {
  return state.printerSchema?.machines.find((m) => m.id === id);
}

/** A new printer profile based on a machine, saved. */
async function makePrinter(model, name) {
  const settings = await engine.newPrinter(model);
  return library.printers.save({ type: 'printer', name: name || machine(model)?.name || model, settings });
}

/** Uses a printer for every project. */
async function usePrinter(printer) {
  if (state.printer) saveNow();
  state.printer = printer;
  library.printers.setCurrent(printer.id);
  renderPrinterBar();
  state.refit = true;
  await run();
}

/** The printer in use last time, or any; the first time, one profile for each machine. */
async function choosePrinter() {
  if (!library.printers.list().length) {
    let first = null;
    for (const m of state.printerSchema.machines) {
      const p = await makePrinter(m.id, m.name.replace(/ \(.*\)$/, ''));
      first ??= p;
    }
    return first;
  }
  const id = library.printers.current();
  return (id && library.printers.load(id)) || library.printers.all().at(-1);
}

function renderPrinterBar() {
  const select = /** @type {HTMLSelectElement} */ ($('printerSelect'));
  select.replaceChildren(
    ...library.printers.list().sort((a, b) => a.name.localeCompare(b.name)).map((e) => el('option', { value: e.id, text: e.name || 'Untitled' })),
  );
  select.value = state.printer.id;
  $('printerName').value = state.printer.name || '';
  const m = machine(state.printer.settings.model);
  $('printerBase').textContent = m ? `Based on the ${m.name}: its start and end G-code and its heater limits.` : '';
  $('newPrinterChoice').replaceChildren(
    ...state.printerSchema.machines.map((mm) =>
      button(`+ ${mm.name}`, 'tiny', async () => {
        $('newPrinterChoice').hidden = true;
        await usePrinter(await makePrinter(mm.id, `My ${mm.name.replace(/ \(.*\)$/, '')}`));
      }),
    ),
  );
  renderHeader();
}

// ---------------------------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------------------------

/** @param {string} text @param {'ok' | 'bad' | 'warn' | ''} [kind] */
function setStatus(text, kind = '') {
  const s = $('status');
  s.textContent = text;
  s.className = `status ${kind}`;
}

function kindName(id) {
  return state.kinds.find((k) => k.id === id)?.name || id;
}

function renderHeader() {
  const p = state.project;
  $('projectTitle').textContent = p ? p.name || 'Untitled' : '…';
  $('projectKind').textContent = state.printer ? `on ${state.printer.name || 'a printer'}` : '';
  document.title = p ? `${p.name || 'Untitled'} · ezgc` : 'ezgc';
}

/** @param {{ kind: string, text: string }[]} list */
function renderNotices(list) {
  $('notices').replaceChildren(...list.map((n) => el('p', { className: `notice ${n.kind}`, text: n.text })));
}

/** @param {number[] | null} feed */
function renderLegend(feed) {
  $('legend').hidden = !feed;
  if (!feed) return;
  const same = Math.round(feed[0]) === Math.round(feed[1]);
  $('legendSlow').textContent = same ? '' : fmt(feed[0]);
  $('legendFast').textContent = `${fmt(feed[1])} mm/min`;
}

/** Minutes as "1 h 05 min" / "12 min 30 s". */
function duration(min) {
  if (!(min > 0)) return '0 s';
  const s = Math.round(min * 60);
  if (s >= 3600) return `${Math.floor(s / 3600)} h ${String(Math.round((s % 3600) / 60)).padStart(2, '0')} min`;
  if (s >= 60) return `${Math.floor(s / 60)} min ${String(s % 60).padStart(2, '0')} s`;
  return `${s} s`;
}

/** "12.5 mm³/s", or "10–12.5 mm³/s" when it varies. */
function range(r, digits, unit) {
  if (!r) return '–';
  const [a, b] = [fmt(r[0], digits), fmt(r[1], digits)];
  return a === b ? `${a} ${unit}` : `${a}–${b} ${unit}`;
}

function renderParts(r) {
  const host = $('parts');
  if (!r.parts.length) {
    host.replaceChildren(el('section', { className: 'card' }, [el('p', { className: 'hint', text: 'Nothing to print yet: see the Settings tab.' })]));
    return;
  }
  host.replaceChildren(...r.parts.map((part, i) => partCard(part, i, r.parts.length)));
}

function partCard(part, i, count) {
  const s = part.stats;
  const b = s.bounds;
  const size = b ? `${fmt(b.max[0] - b.min[0], 1)} × ${fmt(b.max[1] - b.min[1], 1)} × ${fmt(b.max[2], 1)} mm` : '–';
  const rows = [
    ['Print time', duration(s.timeMin)],
    ['Size', size],
    ['Layers', fmt(s.layers, Number.isInteger(s.layers) ? 0 : 2)],
    ['Material', `${fmt(s.volumeMm3 / 1000, 1)} cm³`],
    ['Flow', range(s.flow, 1, 'mm³/s')],
    ['Feed', range(s.feed, 0, 'mm/min')],
  ];
  if (part.massG != null) rows.push(['Weight', `${fmt(part.massG, 1)} g`]);
  if (part.cost != null) rows.push(['Cost', fmt(part.cost, 2)]);
  const error = el('p', { className: 'error', role: 'alert', hidden: true });
  const peek = el('pre', { className: 'peek', hidden: true });
  const fileName = () => `${slug(state.project.name)}${part.name ? `-${slug(part.name)}` : ''}.gcode`;
  /** Writes the G-code, or shows why it can't be. */
  async function gcode() {
    error.hidden = true;
    try {
      return await engine.gcode(projectText(), printerText(), i);
    } catch (err) {
      error.textContent = String(err.message || err);
      error.hidden = false;
      return null;
    }
  }
  const buttons = [
    button('Download .gcode', 'primary', async () => {
      const text = await gcode();
      if (text) download(fileName(), text);
    }),
    button('Copy', '', async (btn) => {
      const text = await gcode();
      if (text) flash(btn, (await copyText(text)) ? 'Copied' : 'Copy refused');
    }),
    button('Peek', '', async (btn) => {
      if (!peek.hidden) {
        peek.hidden = true;
        btn.textContent = 'Peek';
        return;
      }
      const text = await gcode();
      if (!text) return;
      const lines = text.split('\n');
      peek.textContent = lines.slice(0, 80).join('\n') + (lines.length > 80 ? `\n… ${fmt(lines.length - 80)} more lines` : '');
      peek.hidden = false;
      btn.textContent = 'Hide';
    }),
  ];
  if (navigator.canShare) {
    buttons.splice(
      1,
      0,
      button('Share', '', async () => {
        const text = await gcode();
        if (!text) return;
        const file = new File([text], fileName(), { type: 'text/plain' });
        if (navigator.canShare({ files: [file] })) navigator.share({ files: [file] }).catch(() => {});
        else download(fileName(), text);
      }),
    );
  }
  const forPrinter = el('button', { className: 'linkish', type: 'button', text: state.printer.name || 'the printer' });
  forPrinter.addEventListener('click', () => selectTab('printer'));
  return el('section', { className: 'card part' }, [
    el('h2', { text: count > 1 ? part.name || `Part ${i + 1}` : 'Print' }),
    el('p', { className: 'hint' }, ['For ', forPrinter]),
    el('dl', { className: 'stats' }, rows.map(([k, v]) => el('div', {}, [el('dt', { text: k }), el('dd', { text: v })]))),
    part.blocked ? el('p', { className: 'notice bad', text: `No G-code yet: ${part.blocked}` }) : null,
    el('div', { className: 'buttons' }, buttons),
    error,
    peek,
  ]);
}

/** @param {string} text @param {string} kind @param {(btn: HTMLButtonElement) => void} onClick */
function button(text, kind, onClick) {
  const b = /** @type {HTMLButtonElement} */ (el('button', { className: `btn ${kind}`, type: 'button', text }));
  b.addEventListener('click', () => onClick(b));
  return b;
}

function flash(btn, text) {
  const was = btn.textContent;
  btn.textContent = text;
  setTimeout(() => (btn.textContent = was), 1200);
}

// ---------------------------------------------------------------------------------------------
// The projects sheet
// ---------------------------------------------------------------------------------------------

function renderProjects() {
  // A blank project of each kind, then the ready-made ones.
  $('kindList').replaceChildren(
    ...[...state.kinds, ...state.templates].map((k) => {
      const card = el('button', { className: 'kind-card', type: 'button' }, [el('b', { text: k.name }), el('span', { text: k.description })]);
      card.addEventListener('click', async () => {
        dialog().close();
        await newProject(k.id);
      });
      return card;
    }),
  );

  const current = state.project?.id;
  const items = library.projects.list().map((e) => {
    const when = new Date(e.modified);
    const open = el('button', { className: 'open', type: 'button' }, [
      el('b', { text: e.name || 'Untitled' }),
      el('span', { text: `${kindName(e.kind)} · ${Number.isNaN(when.getTime()) ? '' : when.toLocaleString()}` }),
    ]);
    open.addEventListener('click', async () => {
      const p = library.projects.load(e.id);
      dialog().close();
      if (p) await openProject(p);
    });
    return el('li', { className: `project-item${e.id === current ? ' current' : ''}` }, [
      open,
      button('Duplicate', 'tiny', async () => {
        if (e.id === current) saveNow();
        const copy = library.projects.duplicate(e.id);
        if (copy) {
          dialog().close();
          await openProject(copy);
        }
      }),
      button('Export', 'tiny', () => {
        if (e.id === current) saveNow();
        const p = library.projects.load(e.id);
        if (p) download(`${slug(p.name)}.ezgc.json`, toFile(p), 'application/json');
      }),
      button('Delete', 'tiny danger', async () => {
        if (!confirm(`Delete “${e.name || 'Untitled'}” from this device? Export it first to keep a copy.`)) return;
        library.projects.remove(e.id);
        if (e.id === current) {
          state.project = null;
          await openSomething();
        }
        renderProjects();
      }),
    ]);
  });
  $('projectList').replaceChildren(...items);
  if (!items.length) $('projectList').append(el('li', { className: 'hint', text: 'None yet.' }));
  $('storageNote').classList.toggle('error', !library.ok);
}

async function openFile(file) {
  const error = $('fileError');
  error.hidden = true;
  const found = parseFile(await file.text());
  if ('error' in found) {
    error.textContent = found.error;
    error.hidden = false;
    return;
  }
  const strip = ({ id, created, modified, app, format, ...item }) => item;
  const printers = found.printers.map((p) => library.printers.save(strip(p)));
  const projects = found.projects.map((p) => library.projects.save(strip(p)));
  dialog().close();
  if (projects.length) await openProject(projects[0]);
  if (printers.length && !projects.length) {
    await usePrinter(printers[0]);
    selectTab('printer');
  } else if (printers.length) renderPrinterBar();
}

// ---------------------------------------------------------------------------------------------
// Wiring
// ---------------------------------------------------------------------------------------------

function selectTab(name) {
  for (const t of TABS) {
    $(`tab-${t}`).setAttribute('aria-selected', String(t === name));
    $(`panel-${t}`).hidden = t !== name;
  }
}
for (const t of TABS) $(`tab-${t}`).addEventListener('click', () => selectTab(t));

$('projectButton').addEventListener('click', () => {
  saveNow();
  renderProjects();
  dialog().showModal();
});
$('closeProjects').addEventListener('click', () => dialog().close());
$('openFile').addEventListener('click', () => $('fileInput').click());
$('fileInput').addEventListener('change', (e) => {
  const input = /** @type {HTMLInputElement} */ (e.target);
  if (input.files?.[0]) openFile(input.files[0]);
  input.value = '';
});
$('exportAll').addEventListener('click', () => {
  saveNow();
  download(`ezgc-backup-${new Date().toISOString().slice(0, 10)}.json`, bundle(library.projects.all(), library.printers.all()), 'application/json');
});
$('projectName').addEventListener('input', (e) => {
  state.project.name = /** @type {HTMLInputElement} */ (e.target).value;
  renderHeader();
  saveSoon();
});
$('projectNotes').addEventListener('input', (e) => {
  state.project.notes = /** @type {HTMLTextAreaElement} */ (e.target).value;
  saveSoon();
});
$('resetSettings').addEventListener('click', () => {
  const schema = state.schemas.get(state.project.kind);
  if (!schema || !confirm('Set every setting of this project back to its default?')) return;
  change(() => (state.project.settings = structuredClone(schema.defaults)));
});

$('printerSelect').addEventListener('change', async (e) => {
  const p = library.printers.load(/** @type {HTMLSelectElement} */ (e.target).value);
  if (p) await usePrinter(p);
});
$('printerName').addEventListener('input', (e) => {
  state.printer.name = /** @type {HTMLInputElement} */ (e.target).value;
  saveNow();
  const option = /** @type {HTMLSelectElement} */ ($('printerSelect')).selectedOptions[0];
  if (option) option.textContent = state.printer.name || 'Untitled';
});
$('newPrinter').addEventListener('click', () => ($('newPrinterChoice').hidden = !$('newPrinterChoice').hidden));
$('duplicatePrinter').addEventListener('click', async () => {
  saveNow();
  const copy = library.printers.duplicate(state.printer.id);
  if (copy) await usePrinter(copy);
});
$('exportPrinter').addEventListener('click', () => {
  saveNow();
  download(`printer-${slug(state.printer.name)}.ezgc.json`, toFile(state.printer), 'application/json');
});
$('deletePrinter').addEventListener('click', async () => {
  if (library.printers.list().length < 2) {
    alert('Keep at least one printer: make another one first.');
    return;
  }
  if (!confirm(`Delete the printer “${state.printer.name || 'Untitled'}” from this device?`)) return;
  library.printers.remove(state.printer.id);
  state.printer = null;
  await usePrinter(await choosePrinter());
});

$('showTravel').addEventListener('change', (e) => viewport.setTravel(/** @type {HTMLInputElement} */ (e.target).checked));
$('refit').addEventListener('click', () => viewport.refit());
window.addEventListener('pagehide', saveNow);
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'hidden') saveNow();
});

if ('serviceWorker' in navigator && location.protocol !== 'file:') {
  // As in EasyGCode: the browser re-fetches sw.js on every load, and an updated worker taking
  // over reloads the page once, so a new release shows up on a plain refresh.
  let refreshing = false;
  if (navigator.serviceWorker.controller) {
    navigator.serviceWorker.addEventListener('controllerchange', () => {
      if (refreshing) return;
      refreshing = true;
      location.reload();
    });
  }
  window.addEventListener('load', () => {
    navigator.serviceWorker
      .register('sw.js', { updateViaCache: 'none' })
      .then((reg) => reg.update())
      .catch(() => {});
  });
}

async function boot() {
  try {
    await engine.ready;
  } catch (err) {
    setStatus('Engine failed', 'bad');
    renderNotices([{ kind: 'bad', text: `The engine couldn't start: ${err.message || err}` }]);
    return;
  }
  state.kinds = await engine.kinds();
  state.templates = await engine.templates();
  state.printerSchema = await engine.printerSchema();
  state.printerForm = createForm($('printerForm'), state.printerSchema.groups, onPrinterChange, { remember: 'ezgc.open.printer' });
  state.printer = await choosePrinter();
  library.printers.setCurrent(state.printer.id);
  renderPrinterBar();
  await openSomething();
}

boot();
