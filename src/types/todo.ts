export type TodoStatus = "pending" | "in_progress" | "completed";

export type TodoPriority = "high" | "medium" | "low";

export interface TodoItem {
  id: string;
  content: string;
  status: TodoStatus;
  priority: TodoPriority;
  createdAt: number;
  updatedAt: number;
  completedAt?: number;
}

export interface TodoList {
  sessionId: string;
  items: TodoItem[];
  updatedAt: number;
}
