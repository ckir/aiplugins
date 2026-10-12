import { describe, expect, it } from "bun:test"
import { countTodos, formatTodos } from "./format"
import type { Todo } from "./schema"

const todos: Todo[] = [
  { content: "a", status: "pending", priority: "high" },
  { content: "b", status: "in_progress", priority: "medium", activeForm: "Working on b" },
  { content: "c", status: "completed", priority: "low" },
  { content: "d", status: "cancelled", priority: "high" },
]

describe("countTodos", () => {
  it("counts each status in a single pass", () => {
    expect(countTodos(todos)).toEqual({
      total: 4,
      completed: 1,
      in_progress: 1,
      pending: 1,
      cancelled: 1,
    })
  })

  it("handles an empty list", () => {
    expect(countTodos([])).toEqual({
      total: 0,
      completed: 0,
      in_progress: 0,
      pending: 0,
      cancelled: 0,
    })
  })
})

describe("formatTodos", () => {
  it("renders header plus one line per todo", () => {
    const text = formatTodos(todos)
    expect(text).toStartWith("Todos 1/4 completed")
    expect(text).toInclude("[ ] (high) a")
    expect(text).toInclude("[>] (medium) b currently: Working on b")
    expect(text).toInclude("[x] (low) c")
    expect(text).toInclude("[-] (high) d")
  })

  it("renders empty lists with header and (empty)", () => {
    expect(formatTodos([])).toBe("Todos 0/0 completed\n(empty)")
  })
})
