export type TopbarActionKey = "create" | "refresh" | "pauseVisible" | "resumeVisible" | "deleteVisible" | "clearTrash";

export interface TopbarActionState {
  disabled?: boolean;
  title?: string;
}

export type TopbarActionStates = Partial<Record<TopbarActionKey, TopbarActionState>>;
