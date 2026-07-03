// Shared helpers for the generic renderers (GenericList/Board/Table). Every
// renderer is driven by an `entityTypes.<type>.display` config from a module
// manifest (docs/MODULES.md §1: `{ title, subtitle?, badge? }`), never by
// hardcoded field names - so a brand-new module's entities render with zero
// bespoke component code.
//
// A display field may be a plain attrs/entity key ('title') or a small
// resolver function `(entity) => value` for derived display values.

export function resolveField(entity, field) {
  if (!field) return undefined;
  if (typeof field === 'function') return field(entity);
  if (field in entity) return entity[field];
  return entity.attrs ? entity.attrs[field] : undefined;
}

export function resolveDisplay(entity, display = {}) {
  return {
    title: resolveField(entity, display.title) ?? entity.title ?? entity.id,
    subtitle: resolveField(entity, display.subtitle),
    badge: resolveField(entity, display.badge),
  };
}
