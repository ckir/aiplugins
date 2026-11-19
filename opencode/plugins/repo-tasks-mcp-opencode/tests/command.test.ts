/**
 * Copyright 2026 Costas Kirgoussios
 * SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
 *
 * Source-available software. It is NOT open source.
 * Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
 * with the additional terms in LICENSE-ADDENDUM (evaluation grant,
 * anti-circumvention). See both files at the repository root.
 */

import { describe, test, expect } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");

describe("the repo-tasks command", () => {
  const text = readFileSync(join(root, "commands", "repo-tasks.md"), "utf8");

  test("is small, has a description, and runs `list` on the binary", () => {
    expect(Buffer.byteLength(text)).toBeLessThan(400);
    expect(text.startsWith("---\ndescription: ")).toBe(true);
    expect(text).toContain("repo-tasks-cc-mcp list");
  });

  test("INSTALL.md carries the same file, because the bundle does not copy commands/", () => {
    const install = readFileSync(join(root, "INSTALL.md"), "utf8");
    expect(install).toContain(text.trimEnd());
    expect(install).toContain("$DEST/command/repo-tasks.md");
  });
});
