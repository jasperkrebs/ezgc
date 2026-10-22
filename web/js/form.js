// Settings windows, drawn from a schema the engine describes (a kind's settings, or the
// printer's): groups of number fields, toggles and dropdowns, each bound to a dotted key in a plain
// settings object. Adding a setting to a kind adds it here: there is no hand-written HTML per
// project. The window is built once per schema; values, visibility, problems and hints are
// updated in place, so typing never loses focus.

import { el, numberField, parseNumber } from './dom.js';

/**
 * @typedef {{ type: 'number', unit: string, min: number, max: number, step: number, integer: boolean }
 *   | { type: 'toggle' } | { type: 'choice', options: { value: string, label: string }[] }} Input
 * @typedef {{ key: string, anyOf: any[] }} When
 * @typedef {{ key: string, label: string, input: Input, help?: string, when?: When }} Field
 * @typedef {{ title: string, fields: Field[], when?: When, open: boolean }} Group
 */

/** The value at a dotted key. */
export function get(values, key) {
  return key.split('.').reduce((v, k) => (v && typeof v === 'object' ? v[k] : undefined), values);
}

/** A copy of `values` with the value at a dotted key replaced. */
export function set(values, key, value) {
  const [head, ...rest] = key.split('.');
  const base = values && typeof values === 'object' ? values : {};
  return { ...base, [head]: rest.length ? set(base[head], rest.join('.'), value) : value };
}

/** @param {When | undefined} when */
const holds = (when, values) => !when || when.anyOf.some((v) => v === get(values, when.key));

/** A number as a person types it: no float noise (0.30000000000000004 → 0.3). */
const show = (v) => (typeof v === 'number' && Number.isFinite(v) ? String(Number(v.toPrecision(12))) : '');

/**
 * Builds a settings window.
 * @param {HTMLElement} host
 * @param {Group[]} groups
 * @param {(key: string, value: any) => void} onChange a field changed to a valid value
 * @param {{ remember?: string }} [opts] remember: a storage key for which groups are open
 */
export function createForm(host, groups, onChange, opts = {}) {
  host.replaceChildren();
  /** @type {Map<string, { field: Field, group: Group, row: HTMLElement, input: HTMLInputElement | HTMLSelectElement, hint: HTMLElement, error: HTMLElement }>} */
  const rows = new Map();
  /** @type {{ group: Group, node: HTMLDetailsElement }[]} */
  const cards = [];
  let opened = {};
  try {
    opened = (opts.remember && JSON.parse(localStorage.getItem(opts.remember) || '{}')) || {};
  } catch {
    opened = {};
  }

  for (const group of groups) {
    const card = /** @type {HTMLDetailsElement} */ (el('details', { className: 'card group', open: opened[group.title] ?? group.open }));
    card.append(el('summary', { text: group.title }));
    card.addEventListener('toggle', () => {
      if (!opts.remember) return;
      opened[group.title] = card.open;
      try {
        localStorage.setItem(opts.remember, JSON.stringify(opened));
      } catch {
        // Storage refused (private mode): the window just doesn't remember.
      }
    });
    for (const field of group.fields) card.append(fieldRow(field, group));
    cards.push({ group, node: card });
    host.append(card);
  }

  /** @param {Field} field @param {Group} group */
  function fieldRow(field, group) {
    const id = `f-${host.id}-${field.key.replace(/\./g, '-')}`;
    const hint = el('p', { className: 'hint field-hint' });
    const error = el('p', { className: 'error field-error', role: 'alert' });
    const label = el('label', { htmlFor: id, text: field.label });
    let input;
    let control;
    const inp = field.input;
    if (inp.type === 'number') {
      const f = numberField({ id, className: 'num' }, inp.min < 0);
      input = f.input;
      const commit = () => {
        const v = parseNumber(input.value);
        if (!Number.isFinite(v)) return;
        onChange(field.key, inp.integer ? Math.round(v) : v);
      };
      input.addEventListener('input', commit);
      input.addEventListener('change', commit);
      control = el('div', { className: 'num-wrap' }, [...f.nodes, inp.unit ? el('span', { className: 'unit', text: inp.unit }) : null]);
    } else if (inp.type === 'toggle') {
      input = /** @type {HTMLInputElement} */ (el('input', { id, type: 'checkbox', className: 'switch' }));
      input.addEventListener('change', () => onChange(field.key, /** @type {HTMLInputElement} */ (input).checked));
      control = input;
    } else {
      input = /** @type {HTMLSelectElement} */ (el('select', { id }, inp.options.map((o) => el('option', { value: o.value, text: o.label }))));
      input.addEventListener('change', () => onChange(field.key, input.value));
      control = input;
    }
    const row = el('div', { className: `field ${inp.type}` }, [
      el('div', { className: 'field-line' }, [label, control]),
      field.help ? el('p', { className: 'hint', text: field.help }) : null,
      hint,
      error,
    ]);
    row.dataset.key = field.key;
    rows.set(field.key, { field, group, row, input, hint, error });
    return row;
  }

  return {
    /**
     * Shows values, hides what doesn't apply to them, and marks problems and hints.
     * @param {any} values
     * @param {{ key: string, message: string }[]} [problems]
     * @param {{ key: string, text: string }[]} [hints]
     */
    update(values, problems = [], hints = []) {
      for (const { group, node } of cards) node.hidden = !holds(group.when, values);
      for (const { field, row, input, hint, error } of rows.values()) {
        row.hidden = !holds(field.when, values);
        const v = get(values, field.key);
        if (field.input.type === 'toggle') /** @type {HTMLInputElement} */ (input).checked = Boolean(v);
        else if (field.input.type === 'choice') input.value = String(v ?? '');
        // Never rewrite a number while it's being typed.
        else if (document.activeElement !== input) input.value = show(v);
        const p = problems.find((x) => x.key === field.key);
        error.textContent = p ? p.message : '';
        error.hidden = !p;
        row.classList.toggle('bad', Boolean(p));
        const h = hints.filter((x) => x.key === field.key).map((x) => x.text);
        hint.textContent = h.join(' · ');
        hint.hidden = h.length === 0;
      }
    },

    /** Opens the group holding a field and focuses it (for "fix this" links). @param {string} key */
    focus(key) {
      const r = rows.get(key);
      if (!r) return;
      const card = cards.find((c) => c.group === r.group);
      if (card) card.node.open = true;
      r.input.focus();
      r.row.scrollIntoView({ block: 'center', behavior: 'smooth' });
    },
  };
}
