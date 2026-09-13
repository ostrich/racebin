import type { Page } from "@playwright/test";

export async function waitForStableDocumentLayout(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await Promise.all([
      document.fonts.load("400 16px 'Racebin Inter'"),
      document.fonts.load("700 16px 'Racebin Inter'")
    ]);
    await document.fonts.ready;

    await new Promise<void>((resolve) => {
      let quietFrames = 0;
      let previousHeight = -1;

      const check = () => {
        const height = document.documentElement.scrollHeight;
        quietFrames = height === previousHeight ? quietFrames + 1 : 0;
        previousHeight = height;

        if (quietFrames >= 4) {
          resolve();
          return;
        }

        requestAnimationFrame(check);
      };

      requestAnimationFrame(check);
    });
  });
}
