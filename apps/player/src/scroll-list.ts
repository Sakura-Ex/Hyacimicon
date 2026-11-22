/** Keeps wheel and mouse-drag scrolling within a list of selectable entries. */
export class ScrollList extends HTMLElement {
  private drag?: { id: number; y: number; top: number; moved: boolean };
  private suppressClick = false;
  private readonly layout = new ResizeObserver(() => this.updateEdges());

  constructor() {
    super();
    this.addEventListener("scroll", () => this.updateEdges(), {
      passive: true,
    });
    this.addEventListener("pointerdown", (event) => {
      this.suppressClick = false;
      if (
        event.pointerType !== "mouse" ||
        event.button !== 0 ||
        this.scrollHeight <= this.clientHeight
      )
        return;
      this.drag = {
        id: event.pointerId,
        y: event.clientY,
        top: this.scrollTop,
        moved: false,
      };
    });
    this.addEventListener("pointermove", (event) => {
      const drag = this.drag;
      if (!drag || event.pointerId !== drag.id) return;
      if (!(event.buttons & 1)) {
        this.finish();
        return;
      }
      const distance = event.clientY - drag.y;
      if (!drag.moved) {
        if (Math.abs(distance) < 4) return;
        drag.moved = true;
        this.setPointerCapture(drag.id);
        this.toggleAttribute("dragging", true);
      }
      this.scrollTop = drag.top - distance;
    });
    this.addEventListener("pointerleave", () => {
      if (!this.drag?.moved) this.finish();
    });
    for (const type of [
      "pointerup",
      "pointercancel",
      "lostpointercapture",
    ] as const) {
      this.addEventListener(type, (event) => {
        if (event.pointerId === this.drag?.id) this.finish();
      });
    }
    this.addEventListener(
      "click",
      (event) => {
        if (this.suppressClick && event.detail > 0) {
          event.preventDefault();
          event.stopImmediatePropagation();
        }
        this.suppressClick = false;
      },
      true,
    );
    this.addEventListener("dragstart", (event) => event.preventDefault());
  }

  connectedCallback() {
    this.layout.observe(this);
    if (this.firstElementChild) this.layout.observe(this.firstElementChild);
    this.updateEdges();
  }

  disconnectedCallback() {
    this.layout.disconnect();
    this.finish();
  }

  private finish() {
    const drag = this.drag;
    if (!drag) return;
    this.drag = undefined;
    this.suppressClick = drag.moved;
    this.removeAttribute("dragging");
    if (this.hasPointerCapture(drag.id)) this.releasePointerCapture(drag.id);
  }

  private updateEdges() {
    const remaining = this.scrollHeight - this.clientHeight - this.scrollTop;
    // Heights are rounded to CSS pixels, while scrollTop can be fractional.
    this.style.setProperty("--fade-top", (this.scrollTop > 1 ? this.scrollTop : 0) + "px");
    this.style.setProperty("--fade-bottom", (remaining > 1 ? remaining : 0) + "px");
    this.toggleAttribute("scrollable", this.scrollHeight > this.clientHeight);
  }
}
customElements.define("scroll-list", ScrollList);
