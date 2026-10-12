import { describe, expect, it } from "bun:test"
import { validateTodos } from "./validate"

describe("validateTodos", () => {
  it("accepts a valid list", () => {
    const result = validateTodos({
      todos: [{ content: "Write code", status: "pending", priority: "high" }],
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.todos).toHaveLength(1)
  })

  it("accepts an empty list (clear semantics)", () => {
    const result = validateTodos({ todos: [] })
    expect(result.ok).toBe(true)
  })

  it("rejects non-object input", () => {
    expect(validateTodos(null).ok).toBe(false)
    expect(validateTodos("x").ok).toBe(false)
  })

  it("rejects missing todos field", () => {
    const result = validateTodos({})
    expect(result.ok).toBe(false)
  })

  it("rejects non-array todos", () => {
    const result = validateTodos({ todos: "nope" })
    expect(result.ok).toBe(false)
  })

  it("rejects blank content", () => {
    const result = validateTodos({
      todos: [{ content: "   ", status: "pending", priority: "high" }],
    })
    expect(result.ok).toBe(false)
  })

  it("rejects unknown status and priority", () => {
    expect(
      validateTodos({ todos: [{ content: "a", status: "done", priority: "high" }] }).ok,
    ).toBe(false)
    expect(
      validateTodos({ todos: [{ content: "a", status: "pending", priority: "urgent" }] }).ok,
    ).toBe(false)
  })

  it("rejects non-string activeForm", () => {
    const result = validateTodos({
      todos: [{ content: "a", status: "pending", priority: "high", activeForm: 42 }],
    })
    expect(result.ok).toBe(false)
  })

  it("trims surrounding whitespace from content", () => {
    const result = validateTodos({
      todos: [{ content: "  Write code  ", status: "pending", priority: "high" }],
    })
    expect(result.ok).toBe(true)
    if (result.ok) expect(result.todos[0]?.content).toBe("Write code")
  })

  it("allows multiple in_progress items (guidance, not enforced)", () => {
    const result = validateTodos({
      todos: [
        { content: "a", status: "in_progress", priority: "high" },
        { content: "b", status: "in_progress", priority: "low" },
      ],
    })
    expect(result.ok).toBe(true)
  })
})
