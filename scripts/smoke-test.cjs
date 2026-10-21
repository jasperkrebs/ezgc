// Smoke test for the web app: serves web/, opens it in headless Chromium at phone and desktop
// sizes, waits for the Rust engine, then works it like a person would: changes settings (with a
// decimal comma), breaks one and fixes it, switches the printer, makes, renames, duplicates and
// reopens projects, saves a project file and opens it again, and downloads G-code, which must
// match what EasyGCode itself writes (fixtures/easygcode/vase-circle). Checks that nothing errored
// and that the service worker caches every shipped file.
//
//   node scripts/smoke-test.cjs [screenshot-dir]
//
// Needs Playwright (`npm i -g playwright`, or NODE_PATH pointing at a global install) and a
// built engine (scripts/build-web.sh). Exits non-zero on any failure.

const { chromium } = require('playwright');
const { spawn } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const root = path.join(__dirname, '..', 'web');
const fixtures = path.join(__dirname, '..', 'fixtures', 'easygcode');
const port = 8000 + Math.floor(Math.random() * 1000);
const shots = process.argv[2];

/** G-code commands without comments or blank lines. */
const commands = (text) =>
  text
    .split('\n')
    .map((l) => l.replace(/;.*/, '').trim())
    .filter(Boolean);

async function waitForServer(url) {
  for (let i = 0; i < 50; i++) {
    try {
      const r = await fetch(url);
      if (r.ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  throw new Error('web server did not start');
}

/** Clicks something that saves a file, and returns the file's name and text. */
async function saveFrom(page, locator) {
  const [download] = await Promise.all([page.waitForEvent('download', { timeout: 30000 }), locator.click()]);
  return { name: download.suggestedFilename(), text: fs.readFileSync(await download.path(), 'utf8') };
}

/** Waits until the engine has answered the latest change. */
async function settled(page) {
  await page.waitForTimeout(50);
  await page.waitForSelector('#status[aria-busy="false"]');
}

/** Every file the app ships (what the service worker must cache). */
function shipped(dir, base = '') {
  return fs.readdirSync(path.join(dir, base), { withFileTypes: true }).flatMap((e) => {
    const rel = path.posix.join(base, e.name);
    if (e.isDirectory()) return shipped(dir, rel);
    return /\.(js|css|html|wasm|png|webmanifest)$/.test(e.name) && e.name !== 'sw.js' ? [rel] : [];
  });
}

(async () => {
  const server = spawn('python3', ['-m', 'http.server', String(port), '--bind', '127.0.0.1'], { cwd: root, stdio: 'ignore' });
  const failures = [];
  const check = (ok, message) => {
    if (!ok) failures.push(message);
  };
  let browser;
  try {
    // The service worker lists every shipped file.
    const sw = fs.readFileSync(path.join(root, 'sw.js'), 'utf8');
    for (const f of shipped(root)) check(sw.includes(`'./${f}'`), `sw.js doesn't cache ${f}`);

    const url = `http://127.0.0.1:${port}/index.html`;
    await waitForServer(url);
    browser = await chromium.launch();
    for (const [name, opts] of [
      ['phone', { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true, deviceScaleFactor: 2 }],
      ['desktop', { viewport: { width: 1366, height: 860 } }],
    ]) {
      const ctx = await browser.newContext({ ...opts, acceptDownloads: true });
      const page = await ctx.newPage();
      page.on('pageerror', (e) => failures.push(`${name}: page error: ${e.message}`));
      page.on('console', (m) => {
        if (m.type() === 'error') failures.push(`${name}: console error: ${m.text()}`);
      });
      await page.goto(url);
      await settled(page);
      const title = await page.textContent('#projectTitle');
      check(title === 'New vase', `${name}: the first project is "${title}"`);
      check(await page.isVisible('#settingsForm select'), `${name}: no settings window`);
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-1-start.png`) });

      // The page never scrolls; the panel does.
      const scroll = await page.evaluate(() => document.scrollingElement.scrollHeight - innerHeight);
      check(scroll <= 1, `${name}: the page scrolls by ${scroll} px`);

      // A setting, typed with a decimal comma.
      const radius = page.locator('#f-settingsForm-shape-circle-radius');
      await radius.fill('22,5');
      await settled(page);
      let project = await page.evaluate(() => JSON.parse(localStorage.getItem(`easygcode2.p.${localStorage.getItem('easygcode2.current')}`) || '{}'));
      await page.waitForTimeout(500); // the autosave
      project = await page.evaluate(() => JSON.parse(localStorage.getItem(`easygcode2.p.${localStorage.getItem('easygcode2.current')}`) || '{}'));
      check(project.settings?.shape?.circle?.radius === 22.5, `${name}: the radius wasn't saved (${JSON.stringify(project.settings?.shape)})`);

      // Another shape shows its own fields.
      await page.selectOption('#f-settingsForm-shape-kind', 'star');
      await settled(page);
      check(await page.isVisible('#f-settingsForm-shape-star-points'), `${name}: the star's fields don't show`);
      check(!(await page.isVisible('#f-settingsForm-shape-circle-radius')), `${name}: the circle's radius still shows`);
      await page.selectOption('#f-settingsForm-shape-kind', 'circle');

      // A bad value is marked, and nothing is printed until it's fixed.
      const layer = page.locator('#f-settingsForm-layerHeight');
      await layer.fill('0');
      await settled(page);
      check((await page.textContent('#status')) === 'Fix settings', `${name}: a bad layer height isn't flagged`);
      check(await page.isVisible('[data-key="layerHeight"] .field-error'), `${name}: the bad field isn't marked`);
      await layer.fill('0.6');
      await radius.fill('30');
      await settled(page);
      check(!/Fix settings/.test(await page.textContent('#status')), `${name}: fixing the layer height didn't print`);

      // EasyGCode's coat hanger defaults at a set feed, 6 mm tall: the G-code must be EasyGCode's.
      await page.locator('#f-settingsForm-height').fill('6');
      await page.selectOption('#f-settingsForm-speed-mode', 'feed');
      await page.locator('#f-settingsForm-speed-feed').fill('1800');
      await settled(page);
      await page.click('#tab-gcode');
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-2-gcode.png`) });
      const gcode = await saveFrom(page, page.getByRole('button', { name: 'Download .gcode' }));
      check(gcode.name === 'new-vase.gcode', `${name}: the G-code file is called ${gcode.name}`);
      const want = commands(fs.readFileSync(path.join(fixtures, 'vase-circle.gcode'), 'utf8'));
      const got = commands(gcode.text);
      const differs = want.findIndex((c, i) => got[i] !== c);
      check(got.length === want.length && differs === -1, `${name}: the G-code differs from EasyGCode's at command ${differs}: ${got[differs]} / ${want[differs]}`);

      // The printer: the P1P, whose bed this vase doesn't fit at the G1's placement… it does at 125.
      await page.click('#tab-printer');
      await page.selectOption('#f-printerForm-model', 'p1p');
      await settled(page);
      check(await page.isVisible('#f-printerForm-filament-nozzle'), `${name}: no filament temperatures for the P1P`);
      check(!(await page.isVisible('#f-printerForm-pellet-up')), `${name}: pellet zones still shown for the P1P`);
      await page.click('#printerForm summary:has-text("Placement")');
      await page.locator('#f-printerForm-centerX').fill('10');
      await settled(page);
      await page.click('#tab-gcode');
      check(await page.isVisible('text=No G-code yet: the toolpath leaves the bed'), `${name}: a part off the bed isn't blocked`);
      await page.click('#tab-printer');
      await page.locator('#f-printerForm-centerX').fill('128');
      await settled(page);
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-3-printer.png`) });

      // Projects: rename, make another, duplicate, reopen.
      await page.click('#tab-settings');
      await page.click('#settingsForm ~ details summary');
      await page.fill('#projectName', 'Test vase');
      await page.waitForTimeout(500);
      check((await page.textContent('#projectTitle')) === 'Test vase', `${name}: the header doesn't follow the name`);
      await page.click('#projectButton');
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-4-projects.png`) });
      await page.click('.kind-card');
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'New vase', `${name}: no new project`);
      const printer = await page.inputValue('#f-printerForm-model');
      check(printer === 'p1p', `${name}: a new project doesn't start from the last printer (${printer})`);
      await page.click('#projectButton');
      const items = page.locator('.project-item');
      check((await items.count()) === 2, `${name}: ${await items.count()} projects listed, not 2`);
      await items.filter({ hasText: 'Test vase' }).getByRole('button', { name: 'Duplicate' }).click();
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Test vase (copy)', `${name}: duplicating didn't open the copy`);

      // A project file out, and in again.
      await page.click('#projectButton');
      const file = await saveFrom(page, page.locator('.project-item').filter({ hasText: 'Test vase (copy)' }).getByRole('button', { name: 'Export' }));
      const parsed = JSON.parse(file.text);
      check(parsed.kind === 'vase' && parsed.format === 2 && !('id' in parsed), `${name}: the project file is ${file.text.slice(0, 80)}`);
      const saved = path.join(require('node:os').tmpdir(), `easygcode2-${name}.json`);
      fs.writeFileSync(saved, JSON.stringify({ ...parsed, name: 'Imported vase' }));
      await page.setInputFiles('#fileInput', saved);
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Imported vase', `${name}: opening a file didn't open it`);
      check((await page.inputValue('#f-settingsForm-height')) === '6', `${name}: the opened file lost its settings`);

      // An old EasyGCode settings file gets a message.
      await page.click('#projectButton');
      fs.writeFileSync(saved, JSON.stringify({ app: 'easygcode', version: 1, settings: { circle_radius: '30' } }));
      await page.setInputFiles('#fileInput', saved);
      await page.waitForSelector('#fileError:not([hidden])', { timeout: 5000 }).catch(() => {});
      check(/EasyGCode 1 settings file/.test(await page.textContent('#fileError')), `${name}: no message for an EasyGCode 1 file`);
      await page.click('#closeProjects');

      // Everything is still there after a reload.
      await page.reload();
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Imported vase', `${name}: the open project was forgotten`);
      await page.click('#projectButton');
      check((await page.locator('.project-item').count()) === 4, `${name}: the library lost projects`);
      await page.click('#closeProjects');
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-5-reloaded.png`) });
      await ctx.close();
    }
  } catch (err) {
    failures.push(`smoke test crashed: ${err.stack || err}`);
  } finally {
    if (browser) await browser.close();
    server.kill();
  }
  if (failures.length) {
    console.error(failures.join('\n'));
    process.exit(1);
  }
  console.log('smoke test passed');
})();
