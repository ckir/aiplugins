/**
 * Copyright 2026 Costas Kirgoussios
 * SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
 *
 * Source-available software. It is NOT open source.
 * Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
 * with the additional terms in LICENSE-ADDENDUM (evaluation grant,
 * anti-circumvention). See both files at the repository root.
 */

import { describe, expect, it } from "bun:test"
import { parseTodo } from "./schema"

describe("parseTodo", () => {
  it("parses a valid todo with activeForm", () => {
    expect(
      parseTodo({ content: "a", status: "pending", priority: "high", activeForm: "Doing a" }),
    ).toEqual({ content: "a", status: "pending", priority: "high", activeForm: "Doing a" })
  })

  it("parses a valid todo without activeForm", () => {
    expect(parseTodo({ content: "a", status: "completed", priority: "low" })).toEqual({
      content: "a",
      status: "completed",
      priority: "low",
    })
  })

  it("rejects blank content, bad status, bad priority", () => {
    expect(parseTodo({ content: "  ", status: "pending", priority: "high" })).toBeUndefined()
    expect(parseTodo({ content: "a", status: "done", priority: "high" })).toBeUndefined()
    expect(parseTodo({ content: "a", status: "pending", priority: "urgent" })).toBeUndefined()
  })

  it("rejects non-objects and missing fields", () => {
    expect(parseTodo(null)).toBeUndefined()
    expect(parseTodo("a")).toBeUndefined()
    expect(parseTodo({ content: "a", status: "pending" })).toBeUndefined()
  })

  it("rejects non-string activeForm", () => {
    expect(
      parseTodo({ content: "a", status: "pending", priority: "high", activeForm: 7 }),
    ).toBeUndefined()
  })
})
