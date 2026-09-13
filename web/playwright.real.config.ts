import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/real",
  globalTeardown: "./e2e/real/teardown.ts",
  fullyParallel: false,
  // The real-stack files intentionally share one disposable server and database.
  // Serialize them so one worker cannot tear down or mutate shared state while
  // another worker is still exercising it.
  workers: 1,
  reporter: process.env.CI
    ? [["line"], ["github"], ["json", { outputFile: "test-results/results.json" }]]
    : "list",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure"
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "../scripts/run-real-stack-test-server.sh",
    url: "http://127.0.0.1:4174/readyz",
    reuseExistingServer: false,
    // Includes a clean frontend build, Rust relink, migrations, and Argon2
    // account bootstrap before the state-based readiness probe can succeed.
    timeout: 60_000
  }
});
