// Settings windows, drawn from a schema the engine describes (a kind's settings, or the
// printer's): groups of number fields, toggles, dropdowns and lists of points (a body profile),
// each bound to a dotted key in a plain
// settings object. Adding a setting to a kind adds it here: there is no hand-written HTML per
// project. The window is built once per schema; values, visibility, problems and hints are
// updated in place, so typing never loses focus.

import { el, numberField, parseNumber } from './dom.js';

/**
 * @typedef {{ label: string, unit: string, min: number, max: number }} Axis
 * @typedef {{ type: 'number', unit: string, min: number, max: number, step: number, integer: boolean }
 *   | { type: 'toggle' } | { type: 'choice', options: { value: string, label: string }[] }
 *   | { type: 'points', x: Axis, y: Axis, min: number, max: number }} Input
 * @typedef {{ key: string, anyOf: any[], or?: When }} When
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
const holds = (when, values) => !when || when.anyOf.some((v) => v === get(values, when.key)) || Boolean(when.or && holds(when.or, values));

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
  /** @type {Map<string, { field: Field, group: Group, row: HTMLElement, input: HTMLInputElement | HTMLSelectElement, hint: HTMLElement, error: HTMLElement, show?: (v: any) => void }>} */
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
    let showValue;
    const inp = field.input;
    if (inp.type === 'points') {
      const list = pointsList(id, inp, (points) => onChange(field.key, points));
      input = /** @type {any} */ ({ focus: () => list.focusable.focus() });
      control = null;
      showValue = list.show;
      const row = el('div', { className: 'field points' }, [
        el('div', { className: 'field-line' }, [el('span', { className: 'label', text: field.label })]),
        field.help ? el('p', { className: 'hint', text: field.help }) : null,
        list.node,
        hint,
        error,
      ]);
      row.dataset.key = field.key;
      rows.set(field.key, { field, group, row, input, hint, error, show: showValue });
      return row;
    }
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
      for (const { field, row, input, hint, error, show: showValue } of rows.values()) {
        row.hidden = !holds(field.when, values);
        const v = get(values, field.key);
        if (showValue) showValue(v);
        else if (field.input.type === 'toggle') /** @type {HTMLInputElement} */ (input).checked = Boolean(v);
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

/**
 * A list of points (x, y) to edit: a row of two numbers per point, a button to remove it, and one
 * to add a point in the widest gap. Calls `onChange` with the whole list when every row reads as
 * numbers. Rows are redrawn from the values only while none of them is being typed in.
 * @param {string} id
 * @param {{ x: Axis, y: Axis, min: number, max: number }} inp
 * @param {(points: number[][]) => void} onChange
 */
function pointsList(id, inp, onChange) {
  const body = el('div', { className: 'point-rows' });
  const add = el('button', { className: 'btn tiny', type: 'button', text: '+ Add point' });
  const node = el('div', { className: 'point-list', id }, [body, add]);
  /** @type {number[][]} */
  let points = [];
  let shown = '';

  const axisField = (axis, value, onInput) => {
    const f = numberField({ className: 'num', 'aria-label': axis.label }, axis.min < 0);
    f.input.value = show(value);
    f.input.addEventListener('input', onInput);
    f.input.addEventListener('change', onInput);
    return { input: f.input, node: el('label', { className: 'point-axis' }, [el('span', { text: axis.label }), ...f.nodes, axis.unit ? el('span', { className: 'unit', text: axis.unit }) : null]) };
  };

  function draw() {
    const inputs = [];
    body.replaceChildren(
      ...points.map((p, i) => {
        const commit = () => {
          const next = inputs.map(([a, b]) => [parseNumber(a.value), parseNumber(b.value)]);
          if (next.every(([a, b]) => Number.isFinite(a) && Number.isFinite(b))) {
            points = next;
            onChange(points);
          }
        };
        const x = axisField(inp.x, p[0], commit);
        const y = axisField(inp.y, p[1], commit);
        inputs.push([x.input, y.input]);
        const remove = el('button', { className: 'btn tiny', type: 'button', text: '×', title: 'Remove this point', 'aria-label': `Remove point ${i + 1}` });
        remove.disabled = points.length <= inp.min;
        remove.addEventListener('click', () => {
          points = points.filter((_, k) => k !== i);
          draw();
          onChange(points);
        });
        return el('div', { className: 'point-row' }, [x.node, y.node, remove]);
      }),
    );
    add.disabled = points.length >= inp.max;
  }

  add.addEventListener('click', () => {
    // Halfway along the widest gap between neighbouring points (or after the last one).
    const sorted = [...points].sort((a, b) => a[0] - b[0]);
    let best = null;
    for (let i = 1; i < sorted.length; i++) {
      const gap = sorted[i][0] - sorted[i - 1][0];
      if (!best || gap > best.gap) best = { gap, at: [(sorted[i][0] + sorted[i - 1][0]) / 2, (sorted[i][1] + sorted[i - 1][1]) / 2] };
    }
    const last = sorted[sorted.length - 1] || [inp.x.min, (inp.y.min + inp.y.max) / 2];
    const at = best && best.gap > 0 ? best.at : [Math.min(inp.x.max, last[0] + (inp.x.max - inp.x.min) / 10), last[1]];
    points = [...sorted, at].sort((a, b) => a[0] - b[0]).map(([a, b]) => [Number(a.toPrecision(6)), Number(b.toPrecision(6))]);
    draw();
    onChange(points);
  });

  return {
    node,
    /** The element to focus for "fix this". */
    get focusable() {
      return /** @type {HTMLInputElement} */ (body.querySelector('input') || add);
    },
    /** @param {any} v */
    show(v) {
      const list = Array.isArray(v) ? v.filter((p) => Array.isArray(p) && p.length === 2) : [];
      const text = JSON.stringify(list);
      if (text === shown || node.contains(document.activeElement)) return;
      shown = text;
      points = list.map((p) => [Number(p[0]), Number(p[1])]);
      draw();
    },
  };
}
