import { Plugin } from "@opencode/plugin"
import { registerTodoWrite } from "./tool"
import { formatTodos } from "./tool/format"
import { loadTodos } from "./tool/store"

export default Plugin.define({
  id: "todov2-opencode",
  async setup(ctx) {
    if (ctx.options.enabled === false) return
    await ctx.tool.transform((draft) => draft.add(registerTodoWrite(ctx)))
    if (ctx.options.injectEveryRound === false) return
    await ctx.session.hook("context", async (event) => {
      const todos = await loadTodos(ctx, event.sessionID)
      if (todos.length === 0) return
      event.system.push({ type: "text", text: formatTodos(todos) })
    })
  },
})
