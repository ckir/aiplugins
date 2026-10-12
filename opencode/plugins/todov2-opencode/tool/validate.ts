import { isTodoPriority, isTodoStatus, type Todo } from "./schema"

export type ValidateTodosResult =
  | { ok: true; todos: Todo[] }
  | { ok: false; error: string }

/**
 * Dependency-free validation of todowrite input against the documented
 * schema. Returns the first failure as a clear message, or the parsed
 * todos when everything checks out.
 */
export function validateTodos(input: unknown): ValidateTodosResult {
  if (!input || typeof input !== "object") {
    return { ok: false, error: "input must be an object" }
  }
  if (!("todos" in input)) {
    return { ok: false, error: "missing required field: todos" }
  }
  if (!Array.isArray(input.todos)) {
    return { ok: false, error: "todos must be an array" }
  }
  const todos: Todo[] = []
  for (let index = 0; index < input.todos.length; index++) {
    const result = validateTodoItem(input.todos[index], index)
    if (!result.ok) return result
    todos.push(result.todo)
  }
  return { ok: true, todos }
}

function validateTodoItem(
  value: unknown,
  index: number,
): { ok: true; todo: Todo } | { ok: false; error: string } {
  const label = `todos[${index}]`
  if (!value || typeof value !== "object") {
    return { ok: false, error: `${label} must be an object` }
  }
  const item = value as Record<string, unknown>
  if (typeof item.content !== "string" || item.content.trim() === "") {
    return { ok: false, error: `${label}.content must be a non-empty string` }
  }
  if (typeof item.status !== "string" || !isTodoStatus(item.status)) {
    return {
      ok: false,
      error: `${label}.status must be one of pending, in_progress, completed, cancelled`,
    }
  }
  if (typeof item.priority !== "string" || !isTodoPriority(item.priority)) {
    return {
      ok: false,
      error: `${label}.priority must be one of high, medium, low`,
    }
  }
  if (item.activeForm !== undefined && typeof item.activeForm !== "string") {
    return { ok: false, error: `${label}.activeForm must be a string when present` }
  }
  return {
    ok: true,
    todo: {
      content: item.content.trim(),
      status: item.status,
      priority: item.priority,
      ...(typeof item.activeForm === "string" ? { activeForm: item.activeForm } : {}),
    },
  }
}
