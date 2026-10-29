/** Opens a dialog modally, then returns focus to where it was once the dialog goes. */
export function openDialog(element: HTMLDialogElement) {
  const previous = document.activeElement;
  element.showModal();
  return {
    destroy() {
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus();
    },
  };
}
