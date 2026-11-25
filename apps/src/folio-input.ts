/** Keep native browser file navigation and image dragging out of the book, while allowing text editing. */
export function installFolioInputGuards(container: Element) {
  function editing(target: EventTarget | null) {
    return target instanceof Element && target.closest('input, textarea, [contenteditable]:not([contenteditable="false"])') !== null;
  }
  container.addEventListener("contextmenu", event => {
    if (!editing(event.target)) event.preventDefault();
  });
  container.addEventListener("dragstart", event => {
    if (event.target instanceof HTMLImageElement) event.preventDefault();
  });
  window.addEventListener("keydown", event => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a" && !editing(event.target)) event.preventDefault();
  });
  for (const type of ["dragover", "drop"] as const) {
    window.addEventListener(type, event => {
      if (event.dataTransfer?.types.includes("Files")) event.preventDefault();
    });
  }
}
