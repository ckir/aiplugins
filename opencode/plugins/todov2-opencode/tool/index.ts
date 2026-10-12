import type { Plugin } from "@opencode/plugin"
import { countTodos, formatTodos } from "./format"
import { inputSchema, outputSchema } from "./schema"
import { saveTodos } from "./store"
import { validateTodos } from "./validate"

export function registerTodoWrite(ctx: Plugin.Context) {
  return {
    name: "todowrite",
    description:
      "Manage a structured todo list for the current session. Replaces the full list each call. Use it to track progress during multi-step work and keep todo statuses current. Statuses: pending, in_progress (aim for exactly one at a time), completed, cancelled. Priorities: high, medium, low.",
    input: inputSchema,
    output: outputSchema,
    options: { codemode: false, permission: "todowrite" },
    async execute(input: unknown, toolCtx: { sessionID: string }) {
      const result = validateTodos(input)
      if (!result.ok) {
        return {
          content: "Invalid todowrite input: " + result.error,
        }
      }
      const todos = result.todos
      await saveTodos(ctx, toolCtx.sessionID, todos)
      return {
        output: { todos },
        content: formatTodos(todos),
        metadata: { todos, ...countTodos(todos) },
      }
    },
  }
}
