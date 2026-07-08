interface ResizeHandleProps {
  onResize: (deltaX: number) => void;
}

export const ResizeHandle = (props: ResizeHandleProps) => {
  const onMouseDown = (e: MouseEvent) => {
    e.preventDefault();
    let startX = e.clientX;
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";

    // Create an invisible overlay to capture mouse movements outside the handle's narrow 1px width,
    // preventing the cursor from "slipping off" the handle during resize.
    const overlay = document.createElement("div");
    overlay.style.cssText =
      "position:fixed;inset:0;z-index:9999;cursor:col-resize;";
    document.body.appendChild(overlay);

    const onMouseMove = (moveEvent: MouseEvent) => {
      const deltaX = moveEvent.clientX - startX;
      startX = moveEvent.clientX;
      props.onResize(deltaX);
    };

    const onMouseUp = () => {
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      overlay.remove();
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
  };

  return (
    <div
      class="w-1 hover:w-1.5 bg-transparent hover:bg-brand-500/30 active:bg-brand-500/50 cursor-col-resize transition-all flex-shrink-0 relative group z-10"
      onMouseDown={onMouseDown}
    >
      <div class="absolute inset-y-0 -left-1 -right-1" />
    </div>
  );
};
