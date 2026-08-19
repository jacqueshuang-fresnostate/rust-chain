import { nextTick, onBeforeUnmount, watch, type Ref } from 'vue'

const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',')

export function useModalDialog(
  open: Readonly<Ref<boolean>>,
  dialog: Ref<HTMLElement | null>,
  initialFocusSelector = '[data-dialog-initial]',
): {
  trapFocus: (event: KeyboardEvent, close: () => void) => void
  setReturnFocus: (element: HTMLElement | null) => void
} {
  let returnFocus: HTMLElement | null = null
  let requestedReturnFocus: HTMLElement | null = null
  let previousBodyOverflow = ''

  watch(open, async (isOpen) => {
    if (typeof document === 'undefined') return

    if (isOpen) {
      returnFocus = requestedReturnFocus
        || (document.activeElement instanceof HTMLElement ? document.activeElement : null)
      requestedReturnFocus = null
      previousBodyOverflow = document.body.style.overflow
      document.body.style.overflow = 'hidden'
      await nextTick()
      if (!open.value) return
      const container = dialog.value
      const initialFocus = container?.querySelector<HTMLElement>(initialFocusSelector)
        || container?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR)
      initialFocus?.focus()
      return
    }

    document.body.style.overflow = previousBodyOverflow
    await nextTick()
    if (open.value) return
    returnFocus?.focus()
    returnFocus = null
  })

  onBeforeUnmount(() => {
    if (typeof document !== 'undefined' && open.value) {
      document.body.style.overflow = previousBodyOverflow
    }
  })

  function trapFocus(event: KeyboardEvent, close: () => void): void {
    if (event.key === 'Escape') {
      event.preventDefault()
      close()
      return
    }
    if (event.key !== 'Tab') return

    const container = event.currentTarget instanceof HTMLElement ? event.currentTarget : dialog.value
    const focusable = container
      ? [...container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)]
      : []
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (!first || !last) {
      event.preventDefault()
      container?.focus()
      return
    }
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault()
      last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault()
      first.focus()
    }
  }

  function setReturnFocus(element: HTMLElement | null): void {
    requestedReturnFocus = element
  }

  return { trapFocus, setReturnFocus }
}
