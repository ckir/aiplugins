import { Plugin } from "@opencode/plugin"
import { registerTodoWrite } from "./tool"
import { formatTodos } from "./tool/format"
import { loadTodos } from "./tool/store"

export default Plugin.define({
  id: "todov2-opencode",
  async setup(ctx) {
    if (isDisabled(ctx.options.enabled)) return
    await ctx.tool.transform((draft) => draft.add(registerTodoWrite(ctx)))
    if (isDisabled(ctx.options.injectEveryRound)) return
    await ctx.session.hook("context", async (event) => {
      try {
        // Never break generation: a malformed event or storage blip degrades
        // to "no todos", mirroring the re-ghidra hook guards.
        const system = (event as unknown as { system?: unknown }).system
        if (!Array.isArray(system)) return
        const todos = await loadTodos(ctx, event.sessionID)
        if (todos.length === 0) return
        system.push({ type: "text", text: formatTodos(todos) })
      } catch {
        return
      }
    })
  },
})

/**
 * Option flags may arrive as literal `false`, false-ish strings, or be
 * absent. An explicit disable — literal `false` or a false-ish string like
 * `"false"` (which used to silently enable) — turns the plugin off; anything
 * else enables. Same coercion shape as the rtk plugin's `parseBool`.
 */
function isDisabled(value: unknown): boolean {
  if (value === false) return true
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase()
    return (
      normalized === "0" ||
      normalized === "false" ||
      normalized === "no" ||
      normalized === "off"
    )
  }
  return false
}
