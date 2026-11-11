import { expect, test } from "@playwright/test";
import { writeFile } from "node:fs/promises";
import { mockApi } from "./support/mockApi";

// DOM snapshot tracing can dominate measurements for syntax-highlighted input.
test.use({ trace: "off" });

// Diagnostic, not a wall-clock CI gate: timings depend on the host machine.
test("large-paste editor responsiveness @performance", async ({ page }, testInfo) => {
  test.skip(!process.env.RACEBIN_PERFORMANCE, "Opt in with RACEBIN_PERFORMANCE=1");
  test.setTimeout(120_000);
  await mockApi(page, true);
  const cdp = await page.context().newCDPSession(page);
  const cpuThrottle = Number(process.env.RACEBIN_CPU_THROTTLE ?? 4);
  if (!Number.isFinite(cpuThrottle) || cpuThrottle < 1) throw new Error("Invalid CPU throttle");
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: cpuThrottle });
  const measurements = [];
  try {
    await page.goto("/pastes/new");
    const editor = page.getByRole("textbox", { name: "Paste content", exact: true });
    await expect(editor).toBeVisible();
    const language = page.getByRole("combobox", { name: /Language/ });
    await language.fill("javascript");
    await language.press("Tab");
    await cdp.send("Profiler.enable");
    await cdp.send("Profiler.start");
    for (const lines of [100, 1000, 5000, 15000]) {
      const source = `${"const example = 42;\n".repeat(lines)}// end-${lines}`;
      const start = performance.now();
      // Isolate the application's input/render pipeline from Chromium's
      // native insertText/undo bookkeeping used by Playwright fill().
      await editor.evaluate((element, value) => {
        (element as HTMLTextAreaElement).value = value;
        element.dispatchEvent(new Event("input", { bubbles: true }));
      }, source);
      await expect
        .poll(() =>
          page
            .locator(".code-editor pre")
            .evaluate((element, count) => element.textContent?.endsWith(`// end-${count}\n`), lines)
        )
        .toBe(true);
      measurements.push({ lines, characters: source.length, renderMs: performance.now() - start });
      console.log(measurements.at(-1));
    }
    const profile = await cdp.send("Profiler.stop");
    const profilePath = testInfo.outputPath("editor.cpuprofile");
    await writeFile(profilePath, JSON.stringify(profile.profile));
    await testInfo.attach("editor.cpuprofile", {
      path: profilePath,
      contentType: "application/json"
    });
    await testInfo.attach("editor-performance.json", {
      body: JSON.stringify({ cpuThrottle, measurements }, null, 2),
      contentType: "application/json"
    });
  } finally {
    // The disposable context owns this session; closing it also clears the
    // throttle, without waiting on a renderer that may have become unresponsive.
    await page.context().close();
  }
});
