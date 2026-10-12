import { describe, expect, it } from "bun:test"
import { loadTodos, saveTodos, todosKey } from "./store"
import type { Todo } from "./schema"

function fakeCtx(initial?: unknown) {
  const data = new Map<string, unknown>()
  if (initial !== undefined) data.set(todosKey("s1"), initial)
  const storage = {
    async get(key: string) {
      if (key === "boom") throw new Error("storage failure")
      return data.get(key)
    },
    async set(key: string, value: unknown) {
      data.set(key, value)
    },
  }
  return {
    storage,
    sessionID: "s1",
  } as unknown as Parameters<typeof saveTodos>[0]
}

describe("todosKey", () => {
  it("isolates sessions by id", () => {
    expect(todosKey("abc")).toBe("todos/abc")
    expect(todosKey("abc")).not.toBe(todosKey("def"))
  })
})

describe("saveTodos / loadTodos", () => {
  it("round-trips a list", async () => {
    const ctx = fakeCtx()
    const todos: Todo[] = [{ content: "a", status: "pending", priority: "high" }]
    await saveTodos(ctx, "s1", todos)
    expect(await loadTodos(ctx, "s1")).toEqual(todos)
  })

  it("returns [] when nothing is stored", async () => {
    expect(await loadTodos(fakeCtx(), "missing")).toEqual([])
  })

  it("drops invalid items instead of failing the whole list", async () => {
    const ctx = fakeCtx([
      { content: "ok", status: "pending", priority: "high" },
      { content: "", status: "pending", priority: "high" },
      { content: "bad-status", status: "done", priority: "high" },
    ])
    expect(await loadTodos(ctx, "s1")).toEqual([
      { content: "ok", status: "pending", priority: "high" },
    ])
  })

  it("returns [] on storage failure (safe default for per-round injection)", async () => {
    const ctx = fakeCtx()
    expect(await loadTodos(ctx, "boom")).toEqual([])
  })

  it("stores a snapshot, not a live reference", async () => {
    const ctx = fakeCtx()
    const todos: Todo[] = [{ content: "a", status: "pending", priority: "high" }]
    await saveTodos(ctx, "s1", todos)
    todos[0]!.content = "mutated"
    expect(await loadTodos(ctx, "s1")).toEqual([
      { content: "a", status: "pending", priority: "high" },
    ])
  })
})
