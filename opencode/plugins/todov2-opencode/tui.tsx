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
  const theme = props.context.theme
  const todos = createMemo(() => latestTodos(props.context.data.session.message.list(props.sessionID)))
  const done = createMemo(() => todos().filter((todo) => todo.status === "completed").length)
  const hidden = createMemo(() => todos().length > 0 && todos().every((todo) => todo.status === "completed"))

  return (
    <Show when={!hidden()}>
      <box>
        <box flexDirection="row" gap={1} onMouseDown={() => todos().length > 2 && setOpen((value) => !value)}>
          <Show when={todos().length > 2}>
            <text fg={theme.text.default}>{open() ? "▼" : "▶"}</text>
          </Show>
          <text fg={theme.text.default}>
            <b>Todos</b>
            <span style={{ fg: theme.text.subdued }}>
              {todos().length === 0 ? "" : ` (${done()}/${todos().length} done)`}
            </span>
          </text>
        </box>
        <Show
          when={todos().length > 0}
          fallback={
            <text fg={theme.text.subdued}>none yet</text>
          }
        >
          <Show when={todos().length <= 2 || open()}>
            <For each={todos()}>
              {(todo) => (
                <box flexDirection="row" gap={1} minWidth={0}>
                  <text flexShrink={0} fg={markerColor(theme, todo.status)}>
                    {marker(todo.status)}
                  </text>
                  <text fg={lineColor(theme, todo.status)} wrapMode="none" truncate flexGrow={1} flexShrink={1} minWidth={0}>
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
  for (let index = messages.length - 1; index >= 0; index--) {
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

function markerColor(theme: Plugin.Context["theme"], status: TodoStatus) {
  if (status === "completed") return theme.text.feedback.success.default
  if (status === "in_progress") return theme.text.feedback.warning.default
  if (status === "cancelled") return theme.text.subdued
  return theme.text.default
}

function lineColor(theme: Plugin.Context["theme"], status: TodoStatus) {
  if (status === "completed" || status === "cancelled") return theme.text.subdued
  if (status === "in_progress") return theme.text.feedback.warning.default
  return theme.text.default
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
