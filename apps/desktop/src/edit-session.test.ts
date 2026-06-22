import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { headerEditToggleAction, headerEditToggleLabel } from "./edit-session.ts";

describe("header edit toggle", () => {
  it("enters edit mode from read mode", () => {
    const readState = {
      isEditing: false,
      isSaving: false,
      editSource: "",
      loadedSource: "",
    };

    assert.equal(headerEditToggleAction(readState), "enter-edit");
    assert.equal(headerEditToggleLabel(readState), "Edit");
  });

  it("saves before returning to read mode when the draft changed", () => {
    const dirtyState = {
      isEditing: true,
      isSaving: false,
      editSource: "# Daily\n\nnew line",
      loadedSource: "# Daily",
    };

    assert.equal(headerEditToggleAction(dirtyState), "save-and-read");
    assert.equal(headerEditToggleLabel(dirtyState), "Read");
  });

  it("returns to read mode without saving when the draft is unchanged", () => {
    const cleanState = {
      isEditing: true,
      isSaving: false,
      editSource: "# Daily",
      loadedSource: "# Daily",
    };

    assert.equal(headerEditToggleAction(cleanState), "read");
    assert.equal(headerEditToggleLabel(cleanState), "Read");
  });

  it("does nothing while a save is already running", () => {
    const savingState = {
      isEditing: true,
      isSaving: true,
      editSource: "# Daily\n\nnew line",
      loadedSource: "# Daily",
    };

    assert.equal(headerEditToggleAction(savingState), "noop");
    assert.equal(headerEditToggleLabel(savingState), "Read");
  });
});
