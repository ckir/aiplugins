/**
 * Copyright 2026 Costas Kirgoussios
 * SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
 *
 * Source-available software. It is NOT open source.
 * Licensed under the PolyForm Noncommercial License 1.0.0 (LICENSE) together
 * with the additional terms in LICENSE-ADDENDUM (evaluation grant,
 * anti-circumvention). See both files at the repository root.
 */

import type { Todo } from "./schema"

export type TodoCounts = {
  total: number
  completed: number
  in_progress: number
  pending: number
  cancelled: number
}

export function countTodos(todos: readonly Todo[]): TodoCounts {
  const counts: TodoCounts = { total: 0, completed: 0, in_progress: 0, pending: 0, cancelled: 0 }
  for (const todo of todos) {
    counts.total += 1
    counts[todo.status] += 1
  }
  return counts
}

export function formatTodos(todos: readonly Todo[]) {
  const counts = countTodos(todos)
  const header = `Todos ${counts.completed}/${counts.total} completed`
  if (todos.length === 0) return `${header}\n(empty)`
  return [header, ...todos.map(formatTodo)].join("\n")
}

function formatTodo(todo: Todo) {
  const marker =
    todo.status === "completed"
      ? "[x]"
      : todo.status === "in_progress"
        ? "[>]"
        : todo.status === "cancelled"
          ? "[-]"
          : "[ ]"
  const current = todo.activeForm ? ` currently: ${todo.activeForm}` : ""
  return `${marker} (${todo.priority}) ${todo.content}${current}`
}
