import { rmSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

export default function teardown(): void {
  rmSync(join(tmpdir(), "racebin-real-stack-playwright"), {
    force: true,
    recursive: true
  });
}
