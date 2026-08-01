import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e/real",
  globalTeardown: "./e2e/real/teardown.ts",
  fullyParallel: false,
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:4174",
    trace: "retain-on-failure"
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } }
  ],
  webServer: {
    command: "../scripts/run-real-stack-test-server.sh",
    url: "http://127.0.0.1:4174/healthz",
    reuseExistingServer: false,
    timeout: 30_000
  }
});
