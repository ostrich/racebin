import { appendFileSync, existsSync, readFileSync } from "node:fs";
import process from "node:process";

const reportPath = process.argv[2] ?? "web/test-results/results.json";
if (!existsSync(reportPath)) process.exit(0);

const report = JSON.parse(readFileSync(reportPath, "utf8"));
const failures = [];

function visit(suites, parents = []) {
  for (const suite of suites ?? []) {
    const path = [...parents, suite.title].filter(Boolean);
    for (const spec of suite.specs ?? []) {
      for (const test of spec.tests ?? []) {
        if (test.status === "expected" || test.status === "skipped") continue;
        const result = [...(test.results ?? [])].reverse().find(candidate => candidate.errors?.length);
        const message = result?.errors?.[0]?.message ?? `Unexpected status: ${test.status}`;
        failures.push({ title: [...path, spec.title].join(" › "), message });
      }
    }
    visit(suite.suites, path);
  }
}

visit(report.suites);
if (!failures.length) process.exit(0);

const lines = ["## Browser test failures", ""];
for (const failure of failures) {
  lines.push(`### ${failure.title}`, "", "```text", failure.message.slice(0, 4000), "```", "");
}
const summary = `${lines.join("\n")}\n`;
process.stdout.write(summary);
if (process.env.GITHUB_STEP_SUMMARY) appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary);
