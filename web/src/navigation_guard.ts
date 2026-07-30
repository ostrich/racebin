let isDirty: (() => boolean) | undefined;

export function setUnsavedChangesGuard(check: () => boolean): void {
  isDirty = check;
}

export function clearUnsavedChangesGuard(): void {
  isDirty = undefined;
}

export function hasUnsavedChanges(): boolean {
  return isDirty?.() ?? false;
}

export async function confirmDiscardChanges(): Promise<boolean> {
  if (!hasUnsavedChanges()) return true;
  const dialog = document.createElement("dialog");
  dialog.className = "conversion-dialog";
  dialog.innerHTML = `<div><h2>Discard unsaved changes?</h2>
    <p class="muted">Your changes will not be saved.</p>
    <div class="actions"><button class="button" type="button" data-discard="cancel">Keep editing</button><button class="button danger" type="button" data-discard="confirm">Discard changes</button></div></div>`;
  document.body.append(dialog);
  dialog.showModal();
  return new Promise(resolve => {
    const finish = (confirmed: boolean) => {
      dialog.close();
      dialog.remove();
      resolve(confirmed);
    };
    dialog.addEventListener("click", event => {
      const choice = (event.target as HTMLElement).closest<HTMLElement>("[data-discard]")
        ?.dataset.discard;
      if (choice) finish(choice === "confirm");
    });
    dialog.addEventListener("cancel", event => {
      event.preventDefault();
      finish(false);
    }, { once: true });
  });
}
