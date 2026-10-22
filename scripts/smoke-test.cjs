// Smoke test for the web app: serves web/, opens it in headless Chromium at phone and desktop
// sizes, waits for the Rust engine, then works it like a person would: changes settings (with a
// decimal comma), breaks one and fixes it, downloads G-code (which must be the snapshot's,
// fixtures/snapshots/circle-g1), switches, edits, makes and deletes printers, makes, renames,
// duplicates and reopens projects, and saves project and printer files and opens them again.
// Checks that nothing errored and that the service worker caches every shipped file.
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
const snapshots = path.join(__dirname, '..', 'fixtures', 'snapshots');
const port = 8000 + Math.floor(Math.random() * 1000);
const shots = process.argv[2];

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
      check(title === 'New spiral', `${name}: the first project is "${title}"`);
      check(await page.isVisible('#settingsForm select'), `${name}: no settings window`);
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-1-start.png`) });

      // The page never scrolls; the panel does.
      const scroll = await page.evaluate(() => document.scrollingElement.scrollHeight - innerHeight);
      check(scroll <= 1, `${name}: the page scrolls by ${scroll} px`);

      // A setting, typed with a decimal comma, saved on the device.
      const radius = page.locator('#f-settingsForm-shape-circle-radius');
      await radius.fill('22,5');
      await settled(page);
      await page.waitForTimeout(500); // the autosave
      const project = await page.evaluate(() => JSON.parse(localStorage.getItem(`ezgc.projects.${localStorage.getItem('ezgc.projects.current')}`) || '{}'));
      check(project.settings?.shape?.circle?.radius === 22.5, `${name}: the radius wasn't saved (${JSON.stringify(project.settings?.shape)})`);
      check(!('printer' in project), `${name}: the project holds a printer`);

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

      // Speed is set as flow, with the feed it makes right next to it.
      const flowHint = await page.textContent('[data-key="flow"] .field-hint');
      check(/= 1069 mm\/min/.test(flowHint), `${name}: the flow's feed isn't shown (${flowHint})`);

      // The defaults, 6 mm tall, on the G1: the G-code must be the snapshot's.
      await page.locator('#f-settingsForm-height').fill('6');
      await settled(page);
      await page.click('#tab-gcode');
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-2-gcode.png`) });
      const gcode = await saveFrom(page, page.getByRole('button', { name: 'Download .gcode' }));
      check(gcode.name === 'new-spiral.gcode', `${name}: the G-code file is called ${gcode.name}`);
      // The first line names the project; the rest must be identical.
      const want = fs.readFileSync(path.join(snapshots, 'circle-g1.gcode'), 'utf8').split('\n').slice(1);
      const got = gcode.text.split('\n').slice(1);
      const differs = want.findIndex((l, i) => got[i] !== l);
      check(got.length === want.length && differs === -1, `${name}: the G-code differs from the snapshot at line ${differs + 2}: ${got[differs]} / ${want[differs]}`);

      // Printers live on the device: the G1 and the P1P to start with.
      await page.click('#tab-printer');
      const printerNames = await page.locator('#printerSelect option').allTextContents();
      check(JSON.stringify(printerNames) === JSON.stringify(['Bambu P1P', 'Ginger G1']), `${name}: the printers are ${printerNames}`);
      const p1p = await page.locator('#printerSelect option', { hasText: 'Bambu P1P' }).getAttribute('value');
      await page.selectOption('#printerSelect', p1p);
      await settled(page);
      check(await page.isVisible('#f-printerForm-filament-nozzle'), `${name}: no filament temperatures for the P1P`);
      check(!(await page.isVisible('#f-printerForm-pellet-up')), `${name}: pellet zones still shown for the P1P`);
      await page.click('#printerForm summary:has-text("Bed and placement")');
      check((await page.inputValue('#f-printerForm-bed-width')) === '256', `${name}: the P1P's bed isn't its own`);
      await page.locator('#f-printerForm-centerX').fill('10');
      await settled(page);
      await page.click('#tab-gcode');
      check(await page.isVisible('text=No G-code yet: the toolpath leaves the bed'), `${name}: a part off the bed isn't blocked`);
      await page.click('#tab-printer');
      await page.locator('#f-printerForm-centerX').fill('128');
      await page.fill('#printerName', 'My P1P');
      await settled(page);
      check((await page.locator('#printerSelect option:checked').textContent()) === 'My P1P', `${name}: renaming the printer didn't show`);
      // A new printer from a machine, and deleting it again.
      await page.click('#newPrinter');
      await page.click('#newPrinterChoice button:has-text("Ginger G1")');
      await settled(page);
      check((await page.inputValue('#printerName')) === 'My Ginger G1', `${name}: no new printer`);
      check((await page.locator('#printerSelect option').count()) === 3, `${name}: the new printer isn't listed`);
      page.once('dialog', (d) => d.accept());
      await page.click('#deletePrinter');
      await settled(page);
      check((await page.locator('#printerSelect option').count()) === 2, `${name}: deleting a printer didn't`);
      await page.selectOption('#printerSelect', p1p);
      await settled(page);
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-3-printer.png`) });

      // Projects: rename, make another (it prints on the same printer), duplicate, reopen.
      await page.click('#tab-settings');
      await page.click('#settingsForm ~ details summary');
      await page.fill('#projectName', 'Test spiral');
      await page.waitForTimeout(500);
      check((await page.textContent('#projectTitle')) === 'Test spiral', `${name}: the header doesn't follow the name`);
      await page.click('#projectButton');
      if (shots) await page.screenshot({ path: path.join(shots, `${name}-4-projects.png`) });
      await page.click('.kind-card');
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'New spiral', `${name}: no new project`);
      check((await page.locator('#printerSelect option:checked').textContent()) === 'My P1P', `${name}: a new project changed the printer`);
      await page.click('#projectButton');
      const items = page.locator('.project-item');
      check((await items.count()) === 2, `${name}: ${await items.count()} projects listed, not 2`);
      await items.filter({ hasText: 'Test spiral' }).getByRole('button', { name: 'Duplicate' }).click();
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Test spiral (copy)', `${name}: duplicating didn't open the copy`);

      // A project file out, and in again.
      await page.click('#projectButton');
      const file = await saveFrom(page, page.locator('.project-item').filter({ hasText: 'Test spiral (copy)' }).getByRole('button', { name: 'Export' }));
      const parsed = JSON.parse(file.text);
      check(
        parsed.app === 'ezgc' && parsed.kind === 'spiral' && parsed.format === 1 && !('id' in parsed) && !('printer' in parsed),
        `${name}: the project file is ${file.text.slice(0, 120)}`,
      );
      const saved = path.join(require('node:os').tmpdir(), `ezgc-${name}.json`);
      fs.writeFileSync(saved, JSON.stringify({ ...parsed, name: 'Imported spiral' }));
      await page.setInputFiles('#fileInput', saved);
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Imported spiral', `${name}: opening a file didn't open it`);
      check((await page.inputValue('#f-settingsForm-height')) === '6', `${name}: the opened file lost its settings`);

      // A printer file out, and in again as someone else's printer.
      await page.click('#tab-printer');
      const printerFile = await saveFrom(page, page.locator('#exportPrinter'));
      const printerParsed = JSON.parse(printerFile.text);
      check(printerParsed.type === 'printer' && printerParsed.settings?.model === 'p1p', `${name}: the printer file is ${printerFile.text.slice(0, 120)}`);
      fs.writeFileSync(saved, JSON.stringify({ ...printerParsed, name: "Friend's P1P" }));
      await page.click('#projectButton');
      await page.setInputFiles('#fileInput', saved);
      await settled(page);
      check((await page.inputValue('#printerName')) === "Friend's P1P", `${name}: opening a printer file didn't use it`);

      // EasyGCode's old settings files are another app's.
      await page.click('#projectButton');
      fs.writeFileSync(saved, JSON.stringify({ app: 'easygcode', version: 1, settings: { circle_radius: '30' } }));
      await page.setInputFiles('#fileInput', saved);
      await page.waitForSelector('#fileError:not([hidden])', { timeout: 5000 }).catch(() => {});
      check(/another app/.test(await page.textContent('#fileError')), `${name}: no message for another app's file`);
      await page.click('#closeProjects');

      // Everything is still there after a reload.
      await page.reload();
      await settled(page);
      check((await page.textContent('#projectTitle')) === 'Imported spiral', `${name}: the open project was forgotten`);
      check((await page.inputValue('#printerName')) === "Friend's P1P", `${name}: the printer in use was forgotten`);
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
