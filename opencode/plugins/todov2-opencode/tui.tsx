import { Plugin } from "@opencode/plugin/tui"
import { createMemo, createSignal, For, Show } from "solid-js"

const TOOL = "todowrite"

type TodoStatus = "pending" | "in_progress" | "completed" | "cancelled"

type Todo = {
  content: string
  status: TodoStatus
  priority: string
  activeForm?: string
}

function View(props: { context: Plugin.Context; sessionID: string }) {
  const [open, setOpen] = createSignal(true)
  const palette = readPalette(props.context.theme)
  const todos = createMemo(() => {
    try {
      return latestTodos(props.context.data.session.message.list(props.sessionID))
    } catch {
      // Host shape drift must blank the sidebar, never crash it.
      return []
    }
  })
  const done = createMemo(() => todos().filter((todo) => todo.status === "completed").length)
  const hidden = createMemo(() => todos().length > 0 && todos().every((todo) => todo.status === "completed"))

  return (
    <Show when={!hidden()}>
      <box>
        <box flexDirection="row" gap={1} onMouseDown={() => todos().length > 2 && setOpen((value) => !value)}>
          <Show when={todos().length > 2}>
            <text fg={palette.def}>{open() ? "▼" : "▶"}</text>
          </Show>
          <text fg={palette.def}>
            <b>Todos</b>
            <span style={{ fg: palette.subdued }}>
              {todos().length === 0 ? "" : ` (${done()}/${todos().length} done)`}
            </span>
          </text>
        </box>
        <Show
          when={todos().length > 0}
          fallback={
            <text fg={palette.subdued}>none yet</text>
          }
        >
          <Show when={todos().length <= 2 || open()}>
            <For each={todos()}>
              {(todo) => (
                <box flexDirection="row" gap={1} minWidth={0}>
                  <text flexShrink={0} fg={markerColor(palette, todo.status)}>
                    {marker(todo.status)}
                  </text>
                  <text fg={lineColor(palette, todo.status)} wrapMode="none" truncate flexGrow={1} flexShrink={1} minWidth={0}>
                    {todo.content}
                    {todo.priority ? ` (${todo.priority})` : ""}
                    {todo.status === "in_progress" && todo.activeForm ? ` currently: ${todo.activeForm}` : ""}
                  </text>
                </box>
              )}
            </For>
          </Show>
        </Show>
      </box>
    </Show>
  )
}

export default Plugin.define({
  id: "todov2-opencode.tui",
  setup(context) {
    if (context.options.enabled === false) return
    return context.ui.slot({
      append: "sidebar.content",
      render: (props) => <View context={context} sessionID={props.sessionID} />,
    })
  },
})

function latestTodos(messages: ReadonlyArray<{ type?: unknown; content?: unknown }>): Todo[] {
  // Bound the scan: the newest todowrite verdict is near the tail, and a long
  // session would otherwise pay O(history) parsing on every render.
  const start = Math.max(0, messages.length - MAX_SCAN_MESSAGES)
  for (let index = messages.length - 1; index >= start; index--) {
    const message = messages[index]
    if (!message || message.type !== "assistant" || !Array.isArray(message.content)) continue
    for (let partIndex = message.content.length - 1; partIndex >= 0; partIndex--) {
      const todos = todosFromPart(message.content[partIndex])
      if (todos) return todos
    }
  }
  return []
}

function todosFromPart(part: unknown): Todo[] | undefined {
  if (!isRecord(part) || part.type !== "tool" || part.name !== TOOL) return
  const state = isRecord(part.state) ? part.state : undefined
  return (
    todosFromUnknown(state && "output" in state ? state.output : undefined) ??
    todosFromUnknown(part.output) ??
    todosFromUnknown(state?.metadata) ??
    todosFromUnknown(part.metadata) ??
    todosFromContent(state?.content) ??
    todosFromContent(part.content)
  )
}

function todosFromUnknown(value: unknown): Todo[] | undefined {
  if (!value) return
  if (Array.isArray(value)) return parseTodos(value)
  if (!isRecord(value)) return
  if ("todos" in value) return parseTodos(value.todos)
  return parseTodos(value)
}

function todosFromContent(value: unknown) {
  if (typeof value === "string") return parseChecklist(value)
  if (!Array.isArray(value)) return
  const text = value
    .flatMap((item) => (isRecord(item) && item.type === "text" && typeof item.text === "string" ? [item.text] : []))
    .join("\n")
  if (!text) return
  return todosFromUnknown(parseJson(text)) ?? parseChecklist(text)
}

function parseTodos(value: unknown) {
  if (!Array.isArray(value)) return
  const todos = value.flatMap((item) => {
    const todo = parseTodo(item)
    return todo ? [todo] : []
  })
  return todos.length > 0 ? todos : undefined
}

function parseTodo(value: unknown): Todo | undefined {
  if (!isRecord(value) || typeof value.content !== "string" || !value.content.trim()) return
  return {
    content: value.content.trim(),
    status: parseStatus(value.status),
    priority: typeof value.priority === "string" ? value.priority : "",
    activeForm: typeof value.activeForm === "string" ? value.activeForm : undefined,
  }
}

function parseStatus(value: unknown): TodoStatus {
  if (value === "in_progress" || value === "completed" || value === "cancelled") return value
  return "pending"
}

function parseChecklist(text: string) {
  const todos = text.split(/\r?\n/).flatMap((line) => {
    const match = line.match(/^\s*(?:[-*]\s*)?\[([ xX>•✓√\-–])\]\s+(.*\S)\s*$/)
    if (!match) return []
    const mark = match[1] ?? " "
    const rest = match[2] ?? ""
    const priorityMatch = rest.match(/\s+\((high|medium|low)\)\s*$/i)
    return [
      {
        content: (priorityMatch ? rest.slice(0, priorityMatch.index).trim() : rest).trim(),
        status: markStatus(mark),
        priority: priorityMatch?.[1]?.toLowerCase() ?? "",
      } satisfies Todo,
    ]
  })
  return todos.length > 0 ? todos : undefined
}

function markStatus(mark: string): TodoStatus {
  if (mark === "x" || mark === "X" || mark === "✓" || mark === "√") return "completed"
  if (mark === ">" || mark === "•") return "in_progress"
  if (mark === "-" || mark === "–") return "cancelled"
  return "pending"
}

function parseJson(text: string) {
  try {
    return JSON.parse(text) as unknown
  } catch {
    return
  }
}

function marker(status: TodoStatus) {
  if (status === "completed") return "[x]"
  if (status === "in_progress") return "[>]"
  if (status === "cancelled") return "[-]"
  return "[ ]"
}

type Palette = { def: string; subdued: string; ok: string; warn: string }

/** Newest transcript window scanned per render (see latestTodos). */
const MAX_SCAN_MESSAGES = 50

/**
 * Read theme colors defensively: a theme/API shape drift must blank colors,
 * never crash the whole sidebar.
 */
function readPalette(theme: unknown): Palette {
  const blank = { def: "", subdued: "", ok: "", warn: "" }
  try {
    const text = (theme as { text?: unknown })?.text as
      | {
          default?: unknown
          subdued?: unknown
          feedback?: { success?: { default?: unknown }; warning?: { default?: unknown } }
        }
      | undefined
    const str = (value: unknown) => (typeof value === "string" ? value : "")
    return {
      def: str(text?.default),
      subdued: str(text?.subdued),
      ok: str(text?.feedback?.success?.default),
      warn: str(text?.feedback?.warning?.default),
    }
  } catch {
    return blank
  }
}

function markerColor(palette: Palette, status: TodoStatus) {
  if (status === "completed") return palette.ok
  if (status === "in_progress") return palette.warn
  if (status === "cancelled") return palette.subdued
  return palette.def
}

function lineColor(palette: Palette, status: TodoStatus) {
  if (status === "completed" || status === "cancelled") return palette.subdued
  if (status === "in_progress") return palette.warn
  return palette.def
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
