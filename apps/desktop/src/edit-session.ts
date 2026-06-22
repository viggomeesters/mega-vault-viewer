export type HeaderEditToggleAction = "enter-edit" | "read" | "save-and-read" | "noop";

export type HeaderEditToggleState = {
  isEditing: boolean;
  isSaving: boolean;
  editSource: string;
  loadedSource: string;
};

export function isEditDirty(state: Pick<HeaderEditToggleState, "editSource" | "loadedSource">) {
  return state.editSource !== state.loadedSource;
}

export function headerEditToggleAction(state: HeaderEditToggleState): HeaderEditToggleAction {
  if (state.isSaving) {
    return "noop";
  }
  if (!state.isEditing) {
    return "enter-edit";
  }
  if (isEditDirty(state)) {
    return "save-and-read";
  }

  return "read";
}

export function headerEditToggleLabel(state: HeaderEditToggleState) {
  if (!state.isEditing) {
    return "Edit";
  }

  return "Read";
}
