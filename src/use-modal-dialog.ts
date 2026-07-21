import { useCallback, useEffect, useRef, type KeyboardEvent } from "react";

const FOCUSABLE_SELECTOR = [
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  "a[href]",
  "[tabindex]:not([tabindex='-1'])",
].join(",");

export function getWrappedFocusIndex(
  currentIndex: number,
  count: number,
  backwards: boolean,
) {
  if (count <= 0) return -1;
  if (currentIndex < 0) return backwards ? count - 1 : 0;
  if (backwards) return currentIndex === 0 ? count - 1 : currentIndex - 1;
  return currentIndex === count - 1 ? 0 : currentIndex + 1;
}

export function scheduleFocusRestoration(
  target: HTMLElement | null,
  schedule: (callback: FrameRequestCallback) => number = requestAnimationFrame,
) {
  if (!target) return null;
  return schedule(() => {
    if (target.isConnected) target.focus();
  });
}

export function resolveDialogOpener(
  explicitOpener: HTMLElement | null,
  activeElement: Element | null,
) {
  if (explicitOpener) return explicitOpener;
  if (activeElement && "focus" in activeElement && typeof activeElement.focus === "function") {
    return activeElement as HTMLElement;
  }
  return null;
}

function getFocusableElements(container: HTMLElement) {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR))
    .filter((element) => element.getClientRects().length > 0);
}

export function useModalDialog({
  open,
  canClose,
  onRequestClose,
}: {
  open: boolean;
  canClose: boolean;
  onRequestClose: () => void;
}) {
  const dialogRef = useRef<HTMLElement | null>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  const rememberOpener = useCallback((opener: HTMLElement | null) => {
    openerRef.current = resolveDialogOpener(opener, document.activeElement);
  }, []);

  useEffect(() => {
    if (!open) return;

    openerRef.current = resolveDialogOpener(openerRef.current, document.activeElement);
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    const frameId = requestAnimationFrame(() => {
      const dialog = dialogRef.current;
      if (!dialog) return;
      const preferred = dialog.querySelector<HTMLElement>("[data-modal-autofocus]");
      (preferred ?? dialog).focus();
    });

    return () => {
      cancelAnimationFrame(frameId);
      document.body.style.overflow = previousOverflow;
      scheduleFocusRestoration(openerRef.current);
      openerRef.current = null;
    };
  }, [open]);

  const onDialogKeyDown = useCallback((event: KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") {
      if (canClose) {
        event.preventDefault();
        onRequestClose();
      }
      return;
    }

    if (event.key !== "Tab" || !dialogRef.current) return;
    const focusable = getFocusableElements(dialogRef.current);
    const currentIndex = focusable.indexOf(document.activeElement as HTMLElement);
    const nextIndex = getWrappedFocusIndex(currentIndex, focusable.length, event.shiftKey);
    if (nextIndex < 0) return;
    event.preventDefault();
    focusable[nextIndex].focus();
  }, [canClose, onRequestClose]);

  return { dialogRef, onDialogKeyDown, rememberOpener };
}
