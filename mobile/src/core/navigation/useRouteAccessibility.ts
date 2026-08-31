import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import type { Ref } from 'vue'
import type { RouteLocationNormalizedLoaded } from 'vue-router'
import {
  ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE,
  ROUTE_MAIN_CONTENT_ID,
  createRouteAccessibilityCoordinator,
  focusPreparedRouteMain,
  routeAccessibilityRenderKey,
  type RouteAccessibilityDocument,
  type RouteAccessibilityElement,
  type RouteAccessibilityLocation,
  type RouteAccessibilityTranslator,
} from './accessibility.ts'

function routeSnapshot(route: RouteLocationNormalizedLoaded): RouteAccessibilityLocation {
  return {
    name: route.name,
    path: route.path,
    fullPath: route.fullPath,
  }
}

export function useRouteAccessibility(input: {
  readonly route: RouteLocationNormalizedLoaded
  readonly locale: Readonly<Ref<unknown>>
  readonly translate: RouteAccessibilityTranslator
}) {
  const announcement = ref('')
  let announcementVersion = 0
  let previousRoute: RouteAccessibilityLocation | null = null
  const documentTarget: RouteAccessibilityDocument | null = typeof document === 'undefined'
    ? null
    : {
        get title() {
          return document.title
        },
        set title(value: string) {
          document.title = value
        },
        getElementById(id) {
          return document.getElementById(id)
        },
        querySelector(selector) {
          return document.querySelector<HTMLElement>(selector)
        },
      }

  const coordinator = documentTarget === null
    ? null
    : createRouteAccessibilityCoordinator({
        document: documentTarget,
        announce(message) {
          const version = ++announcementVersion
          announcement.value = ''
          void nextTick(() => {
            if (version === announcementVersion) announcement.value = message
          })
        },
      })

  const currentRenderKey = computed(() => routeAccessibilityRenderKey(routeSnapshot(input.route)))

  watch(
    () => input.route.fullPath,
    () => {
      const current = routeSnapshot(input.route)
      coordinator?.beginNavigation(current, previousRoute, input.translate)
      previousRoute = current
    },
    { immediate: true },
  )

  watch(input.locale, () => {
    coordinator?.updateDocumentTitle(routeSnapshot(input.route), input.translate)
  })

  function findCurrentRouteLayer(): RouteAccessibilityElement | null {
    if (typeof document === 'undefined') return null
    const elements = document.querySelectorAll<HTMLElement>(`[${ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE}]`)
    return [...elements].find((element) => (
      element.getAttribute(ROUTE_ACCESSIBILITY_KEY_ATTRIBUTE) === currentRenderKey.value
    )) || null
  }

  function handleRouteEntered(element: Element): void {
    coordinator?.completeTransition(element as HTMLElement)
  }

  function focusMainContent(): void {
    if (documentTarget) focusPreparedRouteMain(documentTarget)
  }

  onMounted(async () => {
    await nextTick()
    coordinator?.completeTransition(findCurrentRouteLayer())
  })

  onBeforeUnmount(() => {
    announcementVersion += 1
  })

  return {
    announcement,
    currentRenderKey,
    focusMainContent,
    handleRouteEntered,
    mainContentId: ROUTE_MAIN_CONTENT_ID,
    renderKeyFor(route: RouteLocationNormalizedLoaded): string {
      return routeAccessibilityRenderKey(routeSnapshot(route))
    },
  }
}
