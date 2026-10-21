// Generates golden G-code fixtures by running EasyGCode's own generator (gcode.js) headlessly,
// the way EasyGCode's CLAUDE.md describes. The engine's golden tests compare their G-code with
// these, command for command.
//
//   node scripts/easygcode-fixtures.cjs <path to an easygcode checkout>
//
// Writes fixtures/easygcode/<case>.json (EasyGCode's own settings), <case>.project.json (the
// same settings as an EasyGCode 2 project) and <case>.gcode, and records the EasyGCode commit in
// fixtures/easygcode/SOURCE. EasyGCode itself is only read, never changed.

const fs = require('node:fs');
const path = require('node:path');
const { execSync } = require('node:child_process');

const src = process.argv[2];
if (!src) {
  console.error('usage: node scripts/easygcode-fixtures.cjs <path to easygcode>');
  process.exit(2);
}
global.window = {};
require(path.resolve(src, 'geometry.js'));
require(path.resolve(src, 'gcode.js'));
const { GcodeGen } = global.window;

const printer = (over = {}) => ({
  mode: 'pellet',
  multiplier: 1,
  includeStartEnd: true,
  endLift: 50,
  filament: { diameter: 1.75, nozzle: 205, bed: 60, fan: 100 },
  pellet: { up: 210, mid: 210, down: 210, bed: 50, pa: 0.3, purge: 5000, fan: 0 },
  ...over,
});

const SHAPES = {
  circle: { radius: 30 },
  roundedRect: { width: 60, length: 40, fillet: 8 },
  ellipse: { rx: 40, ry: 25 },
  polygon: { radius: 30, sides: 6 },
  star: { outerR: 35, innerR: 18, points: 5 },
  squircle: { size: 30, n: 4 },
};

/** EasyGCode's coat hanger with every extra (pattern, wall hanger, brim, bottom fillet) off. */
function coatHanger(over) {
  const shape = over.shape || 'circle';
  return {
    project: 'cordhanger',
    shape,
    shapeParams: { ...SHAPES[shape], ...(over.shapeParams || {}) },
    printer: printer(over.printer),
    materialDensity: 0,
    materialPrice: 0,
    layerHeight: 0.6,
    lineWidth: 2,
    totalHeight: 6,
    bottomFillet: 0,
    printFeed: 1800,
    flowFeed: { enabled: false, rate: 20 },
    travelFeed: 6000,
    accelToWallFeed: 0,
    tolerance: 0.05,
    seamSide: 'back',
    printDirection: 'ccw',
    centerX: 125,
    centerY: 125,
    fanMode: 'always',
    topStyle: 'flat',
    brim: { enabled: false, outerStyle: 'normal', linesOuter: 0, linesInner: 0, lineWidth: 2, layerHeight: 0.6, feed: 900, multiplier: 0 },
    hanger: { enabled: false, mode: 'single', size: 10, pocket: 10, gapWidthMM: 10, pocketWidthMM: 10, bottom: 1, transition: 1, bridgeFeed: 0, overhangAngle: 0, overhangFeed: 0, flowMode: false },
    pattern: { enabled: false, type: 'weave', amplitude: 1, zAngle: 0, zAngleLowMM: 0, zAngleLow: 0, coverage: 100, bumpFeed: 0, flowMode: false, plBottom: 0, plTop: 0, bumps: 10, spikeDensity: 0, spikeVar: 0, seed: 0, spikeBalance: false, spikeDwell: 0, spikeOutMotion: 'straight', spikeFeedOut: 0, spikeFeedTip: 0, spikeFeedIn: 0, spikeLineWidth: 0, spikeLayerHeight: 0 },
    ...over,
    printer: printer(over.printer),
    shapeParams: { ...SHAPES[shape], ...(over.shapeParams || {}) },
  };
}

/** The same settings as an EasyGCode 2 project (the vase kind). */
function asProject(name, c) {
  const p = c.shapeParams;
  const shape =
    c.shape === 'star' ? { outer: p.outerR, inner: p.innerR, points: p.points } : { ...p };
  return {
    app: 'easygcode',
    format: 2,
    kind: 'vase',
    kindVersion: 1,
    name,
    settings: {
      shape: { kind: c.shape, [c.shape]: shape },
      height: c.totalHeight,
      layerHeight: c.layerHeight,
      lineWidth: c.lineWidth,
      speed: c.flowFeed.enabled ? { mode: 'flow', flow: c.flowFeed.rate } : { mode: 'feed', feed: c.printFeed },
      top: c.topStyle === 'spiral' ? 'open' : 'flat',
      seam: c.seamSide,
      direction: c.printDirection,
      tolerance: c.tolerance,
    },
    printer: {
      model: c.printer.mode === 'filament' ? 'p1p' : 'g1',
      multiplier: c.printer.multiplier,
      includeStartEnd: c.printer.includeStartEnd,
      endLift: c.printer.endLift,
      travelFeed: c.travelFeed,
      centerX: c.centerX,
      centerY: c.centerY,
      filament: c.printer.filament,
      pellet: c.printer.pellet,
    },
  };
}

const cases = {
  // EasyGCode's coat hanger defaults (a 30 mm circle, Ginger G1), 10 revolutions.
  'vase-circle': coatHanger({}),
  // The P1P: filament start/end, E in mm of filament, multiplier, fan; flow mode; a rounded
  // rectangle (straight sides: the ramp's extra points); clockwise from the front; an open top
  // after 8.5 revolutions.
  'vase-rect-filament': coatHanger({
    shape: 'roundedRect',
    printer: { mode: 'filament', multiplier: 0.95, endLift: 30, filament: { diameter: 1.75, nozzle: 210, bed: 65, fan: 80 } },
    layerHeight: 0.3,
    lineWidth: 0.6,
    totalHeight: 2.55,
    flowFeed: { enabled: true, rate: 8 },
    travelFeed: 9000,
    seamSide: 'front',
    printDirection: 'cw',
    topStyle: 'spiral',
    centerX: 128,
    centerY: 128,
  }),
  // No start/end blocks, a star, a seam on +X, a coarser outline, 3.75 revolutions, flat top.
  'vase-star-bare': coatHanger({
    shape: 'star',
    printer: { includeStartEnd: false },
    layerHeight: 0.4,
    lineWidth: 1.2,
    totalHeight: 1.5,
    flowFeed: { enabled: true, rate: 15 },
    seamSide: 'right',
    tolerance: 0.1,
  }),
  // Pellet temperatures, purge, pressure advance and a fan that comes on after the first turn.
  'vase-ellipse-fan': coatHanger({
    shape: 'ellipse',
    printer: { pellet: { up: 200, mid: 220, down: 230, bed: 60, pa: 0, purge: 3000, fan: 30 }, endLift: 20 },
    layerHeight: 0.8,
    lineWidth: 2.4,
    totalHeight: 2.4,
    printFeed: 2400,
    seamSide: 'left',
  }),
  'vase-polygon': coatHanger({ shape: 'polygon', totalHeight: 1.2 }),
  'vase-squircle': coatHanger({ shape: 'squircle', totalHeight: 1.8, printDirection: 'cw' }),
};

const out = path.resolve(__dirname, '..', 'fixtures', 'easygcode');
fs.mkdirSync(out, { recursive: true });
for (const [name, cfg] of Object.entries(cases)) {
  const r = GcodeGen.generate(cfg);
  if (r.warnings.length) console.warn(name, r.warnings);
  fs.writeFileSync(path.join(out, `${name}.json`), JSON.stringify(cfg, null, 2) + '\n');
  fs.writeFileSync(path.join(out, `${name}.project.json`), JSON.stringify(asProject(name, cfg), null, 2) + '\n');
  // The second line is a timestamp: drop it so regenerating changes nothing.
  const gcode = r.gcode.split('\n').filter((l, i) => !(i === 1 && /^; \d{4}-\d\d-\d\dT/.test(l))).join('\n');
  fs.writeFileSync(path.join(out, `${name}.gcode`), gcode);
  console.log(`${name}: ${gcode.split('\n').length} lines`);
}
let commit = 'unknown';
let remote = 'easygcode';
try {
  commit = execSync('git rev-parse HEAD', { cwd: src }).toString().trim();
  remote = execSync('git remote get-url origin', { cwd: src }).toString().trim().replace(/^.*github\.com[/:]/, '').replace(/\.git$/, '');
} catch {
  // Not a git checkout: the commit stays unknown.
}
fs.writeFileSync(path.join(out, 'SOURCE'), `${remote} ${commit}\n`);
