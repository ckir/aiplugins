/**
 * Copyright 2026 AI Plugins Marketplace Contributors
 * SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
 *
 * Source-available software. It is NOT open source.
 * Licensed under the PolyForm Noncommercial License 1.0.0 together with the
 * additional terms (evaluation grant, anti-circumvention) that apply to this
 * repository. See the LICENSE file at the repository root.
 */

import type { Plugin } from "@opencode/plugin"
import { parseTodo, type Todo } from "./schema"

/**
 * Plugin `ctx.storage` is plugin-scoped, not session-scoped.
 * Keys are `todos/<sessionID>` so concurrent sessions stay isolated.
 * The id is sanitized to a KV-safe alphabet so `/`, `..` or empty ids
 * cannot collide with or span keys.
 */
export function todosKey(sessionID: string) {
  const safe = sessionID.replace(/[^A-Za-z0-9_-]/g, "_") || "unknown"
  return `todos/${safe}`
}

export async function saveTodos(ctx: Plugin.Context, sessionID: string, todos: readonly Todo[]) {
  await ctx.storage.set(todosKey(sessionID), JSON.parse(JSON.stringify(todos)))
}

export async function loadTodos(ctx: Plugin.Context, sessionID: string): Promise<Todo[]> {
  try {
    const value = await ctx.storage.get(todosKey(sessionID))
    if (!Array.isArray(value)) return []
    return value.flatMap((item) => {
      const todo = parseTodo(item)
      return todo ? [todo] : []
    })
  } catch {
    // Intentional safe default: the session hook runs before every LLM
    // round, so a storage blip must degrade to "no todos" rather than
    // break generation.
    return []
  }
}
