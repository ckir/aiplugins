export type TodoStatus = "pending" | "in_progress" | "completed" | "cancelled"
export type TodoPriority = "high" | "medium" | "low"

export type Todo = {
  content: string
  status: TodoStatus
  priority: TodoPriority
  activeForm?: string
}

const todoItemSchema = {
  type: "object",
  properties: {
    content: {
      type: "string",
      description: "Brief description of the task",
    },
    status: {
      type: "string",
      enum: ["pending", "in_progress", "completed", "cancelled"],
      description: "Current status of the task: pending, in_progress, completed, cancelled",
    },
    priority: {
      type: "string",
      enum: ["high", "medium", "low"],
      description: "Priority level of the task: high, medium, low",
    },
    activeForm: {
      type: "string",
      description: "Present-tense description of the work currently underway",
    },
  },
  required: ["content", "status", "priority"],
  additionalProperties: false,
}

export const inputSchema = {
  type: "object",
  properties: {
    todos: {
      type: "array",
      description: "The updated todo list. Replaces the full session list on each call.",
      items: todoItemSchema,
    },
  },
  required: ["todos"],
  additionalProperties: false,
}

export const outputSchema = {
  type: "object",
  properties: {
    todos: {
      type: "array",
      items: todoItemSchema,
    },
  },
  required: ["todos"],
  additionalProperties: false,
}

const statuses: ReadonlySet<string> = new Set(["pending", "in_progress", "completed", "cancelled"])
const priorities: ReadonlySet<string> = new Set(["high", "medium", "low"])

export function isTodoStatus(value: string): value is TodoStatus {
  return statuses.has(value)
}

export function isTodoPriority(value: string): value is TodoPriority {
  return priorities.has(value)
}

export function parseTodo(value: unknown): Todo | undefined {
  if (!value || typeof value !== "object") return
  if (!("content" in value) || !("status" in value) || !("priority" in value)) return
  if (typeof value.content !== "string" || value.content.trim() === "") return
  if (typeof value.status !== "string" || !isTodoStatus(value.status)) return
  if (typeof value.priority !== "string" || !isTodoPriority(value.priority)) return
  if (!("activeForm" in value) || value.activeForm === undefined) {
    return {
      content: value.content,
      status: value.status,
      priority: value.priority,
    }
  }
  if (typeof value.activeForm !== "string") return
  return {
    content: value.content,
    status: value.status,
    priority: value.priority,
    activeForm: value.activeForm,
  }
}
