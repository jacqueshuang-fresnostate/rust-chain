export const PRODUCT_BACKEND_ORIGIN = 'https://hipoex.cllbmz.kdns.fr'

export function resolveProductBackendOrigin(value: unknown): string {
  const configured = typeof value === 'string' ? value.trim() : ''
  return configured || PRODUCT_BACKEND_ORIGIN
}
