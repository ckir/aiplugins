import type { Plugin } from "@opencode/plugin"
import { parseTodo, type Todo } from "./schema"

/**
 * Plugin `ctx.storage` is plugin-scoped, not session-scoped.
 * Keys are `todos/<sessionID>` so concurrent sessions stay isolated.
 */
export function todosKey(sessionID: string) {
  return `todos/${sessionID}`
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
