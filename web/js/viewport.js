// 3D view: three.js drawing on the GPU (WebGL 2). Z is up, like slicers. It draws exactly the
// moves the G-code is written from, coloured by feed as EasyGCode does (blue fastest, red
// slowest), travels faint. It renders only when something changes, which saves battery on phones.

import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { LineSegments2 } from 'three/addons/lines/LineSegments2.js';
import { LineSegmentsGeometry } from 'three/addons/lines/LineSegmentsGeometry.js';
import { LineMaterial } from 'three/addons/lines/LineMaterial.js';

/** @param {string} name CSS custom property, e.g. '--accent' */
function token(name) {
  return getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888888';
}

/**
 * EasyGCode's feed colours: hue 240 (blue) at the fastest feed down to 0 (red) at the slowest.
 * @param {number} t 0 = slowest, 1 = fastest
 * @param {THREE.Color} out
 */
export function feedColor(t, out) {
  return out.setHSL(((240 * Math.max(0, Math.min(1, t))) / 360) % 1, 0.85, 0.55);
}

/** @param {HTMLCanvasElement} canvas */
export function createViewport(canvas) {
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));

  const scene = new THREE.Scene();
  const camera = new THREE.PerspectiveCamera(35, 1, 1, 20000);
  camera.up.set(0, 0, 1);
  camera.position.set(300, -380, 260);

  const controls = new OrbitControls(camera, canvas);
  controls.addEventListener('change', requestRender);

  const pathMaterial = new LineMaterial({ linewidth: 1.6, vertexColors: true, worldUnits: false });
  const travelMaterial = new LineMaterial({ linewidth: 1, transparent: true, opacity: 0.35, worldUnits: false });
  const paths = new THREE.Group();
  scene.add(paths);
  /** @type {THREE.LineSegments | null} */
  let grid = null;

  /** Bed width and depth, mm. */
  let bed = [256, 256];
  /** @type {{ min: number[], max: number[] } | null} */
  let lastBounds = null;
  let fitted = false;
  let frame = 0;
  let showTravel = true;

  function requestRender() {
    if (!frame) frame = requestAnimationFrame(render);
  }

  function render() {
    frame = 0;
    renderer.render(scene, camera);
  }

  function resize() {
    const w = Math.max(1, canvas.clientWidth);
    const h = Math.max(1, canvas.clientHeight);
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();
    pathMaterial.resolution.set(w, h);
    travelMaterial.resolution.set(w, h);
    requestRender();
  }
  new ResizeObserver(resize).observe(canvas);

  /** The bed: its outline and a grid every 10 mm (every 50 mm on a big bed), from 0,0. */
  function rebuildGrid() {
    if (grid) {
      scene.remove(grid);
      grid.geometry.dispose();
      /** @type {THREE.Material} */ (grid.material).dispose();
    }
    const [w, d] = bed;
    const step = Math.max(w, d) > 500 ? 50 : 10;
    const pts = [];
    for (let x = 0; x <= w + 1e-9; x += step) pts.push(x, 0, 0, x, d, 0);
    for (let y = 0; y <= d + 1e-9; y += step) pts.push(0, y, 0, w, y, 0);
    pts.push(w, 0, 0, w, d, 0, 0, d, 0, w, d, 0);
    const g = new THREE.BufferGeometry();
    g.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3));
    grid = new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: new THREE.Color(token('--view-grid')) }));
    scene.add(grid);
  }

  /** @param {THREE.Group} group */
  function clear(group) {
    for (const child of [...group.children]) {
      /** @type {any} */ (child).geometry.dispose();
      group.remove(child);
    }
  }

  /** Frames a box: `{ min: [x, y, z], max: [x, y, z] }` (the bed's middle when there's nothing). */
  function fit(bounds) {
    const b = bounds || { min: [bed[0] / 2 - 50, bed[1] / 2 - 50, 0], max: [bed[0] / 2 + 50, bed[1] / 2 + 50, 60] };
    const size = [0, 1, 2].map((k) => b.max[k] - b.min[k]);
    const r = Math.max(10, Math.hypot(size[0], size[1], size[2]) / 2) * 1.1;
    // The narrower of the two view angles, so it fits across a phone's narrow view too.
    const vertical = THREE.MathUtils.degToRad(camera.fov / 2);
    const half = Math.min(vertical, Math.atan(Math.tan(vertical) * camera.aspect));
    const dist = r / Math.sin(half);
    controls.target.set((b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2);
    const dir = new THREE.Vector3(0.62, -0.78, 0.5).normalize();
    camera.position.copy(controls.target).addScaledVector(dir, dist);
    camera.near = Math.max(0.5, dist / 100);
    camera.far = dist * 20;
    camera.updateProjectionMatrix();
    controls.update();
    requestRender();
  }

  function applyTheme() {
    scene.background = new THREE.Color(token('--view-bg'));
    travelMaterial.color.set(token('--muted'));
    rebuildGrid();
    requestRender();
  }
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyTheme);
  canvas.addEventListener('dblclick', () => fit(lastBounds));

  applyTheme();
  resize();

  return {
    /** @param {number[]} size bed width and depth, mm */
    setBed(size) {
      if (size[0] === bed[0] && size[1] === bed[1]) return;
      bed = [size[0], size[1]];
      rebuildGrid();
      if (!lastBounds) fit(null);
      requestRender();
    },

    /** @param {boolean} on */
    setTravel(on) {
      showTravel = on;
      for (const child of paths.children) if (child.userData.travel) child.visible = on;
      requestRender();
    },

    /**
     * Shows the toolpaths of a run: each part's moves, from one end point to the next. Each part
     * prints on its own, on the bed's centre; here the second and later ones stand to the right of
     * the one before, so a container and its lid are seen side by side.
     * @param {{ positions: Float32Array, feeds: Float32Array, travel: Uint8Array }[]} parts
     * @param {{ refit?: boolean }} [view]
     * @returns {{ feed: number[] | null }} the feed range the colours span (mm/min)
     */
    show(parts, view = {}) {
      clear(paths);
      let lo = Infinity;
      let hi = 0;
      for (const p of parts) {
        for (let i = 1; i < p.feeds.length; i++) {
          if (p.travel[i]) continue;
          lo = Math.min(lo, p.feeds[i]);
          hi = Math.max(hi, p.feeds[i]);
        }
      }
      const span = hi - lo;
      const c = new THREE.Color();
      const min = [Infinity, Infinity, Infinity];
      const max = [-Infinity, -Infinity, -Infinity];
      // Side by side: each part shifted along X to start 10 mm right of the last one's end.
      const shifts = [];
      let right = -Infinity;
      for (const p of parts) {
        let [a, b] = [Infinity, -Infinity];
        for (let i = 0; i < p.feeds.length; i++) {
          a = Math.min(a, p.positions[3 * i]);
          b = Math.max(b, p.positions[3 * i]);
        }
        const shift = Number.isFinite(right) && Number.isFinite(a) ? right + 10 - a : 0;
        shifts.push(shift);
        if (Number.isFinite(b)) right = b + shift;
      }
      parts.forEach((p, k) => {
        const n = p.feeds.length;
        const moves = [];
        const colors = [];
        const travels = [];
        const dx = shifts[k];
        const at = (i) => [p.positions[3 * i] + dx, p.positions[3 * i + 1], p.positions[3 * i + 2]];
        for (let i = 0; i < n; i++) {
          const q = at(i);
          for (let k = 0; k < 3; k++) {
            min[k] = Math.min(min[k], q[k]);
            max[k] = Math.max(max[k], q[k]);
          }
          if (i === 0) continue;
          const from = at(i - 1);
          if (p.travel[i]) {
            travels.push(...from, ...q);
            continue;
          }
          moves.push(...from, ...q);
          feedColor(span > 0 ? (p.feeds[i] - lo) / span : 1, c);
          colors.push(c.r, c.g, c.b, c.r, c.g, c.b);
        }
        if (moves.length) {
          const g = new LineSegmentsGeometry();
          g.setPositions(moves);
          g.setColors(colors);
          paths.add(new LineSegments2(g, pathMaterial));
        }
        if (travels.length) {
          const g = new LineSegmentsGeometry();
          g.setPositions(travels);
          const line = new LineSegments2(g, travelMaterial);
          line.userData.travel = true;
          line.visible = showTravel;
          paths.add(line);
        }
      });
      lastBounds = Number.isFinite(min[0]) ? { min, max } : null;
      if (!fitted || view.refit) {
        fit(lastBounds);
        fitted = Boolean(lastBounds);
      }
      requestRender();
      return { feed: hi > 0 ? [lo, hi] : null };
    },

    /** Frames the last result again. */
    refit() {
      fit(lastBounds);
    },
  };
}
