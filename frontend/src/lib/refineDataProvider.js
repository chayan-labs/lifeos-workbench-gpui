// Refine DataProvider over the generic /api/entity (+/api/edge) routes. One
// dataProvider works for every resource/module/type, matching the "one
// generic schema" model - Refine resources map onto entity `type` values,
// not bespoke backend routes. See docs/PLATFORM-SYSTEMS.md and
// frontend/FRONTEND.md §3.
//
// `resource` is read as `module/type` (e.g. "tasks/task"); a bare resource
// name with no slash is treated as a `module` filter with no `type` filter.

import { apiCall, API_BASE } from './api';

function splitResource(resource) {
  const [module, type] = String(resource).split('/');
  return { module, type };
}

// Refine's CrudFilters -> the flat query params /api/entity understands.
// Only `eq` operators on known columns are supported (the backend has no
// generic filter compiler); anything else is ignored rather than silently
// misapplied.
function filtersToParams(filters = []) {
  const params = {};
  const KNOWN = new Set(['status', 'parent_id', 'module', 'type']);
  for (const f of filters) {
    if (f.operator === 'eq' && KNOWN.has(f.field) && f.value != null) {
      params[f.field] = f.value;
    }
  }
  return params;
}

export const refineDataProvider = {
  getApiUrl: () => API_BASE,

  getList: async ({ resource, pagination, filters }) => {
    const { module, type } = splitResource(resource);
    const current = pagination?.current ?? 1;
    const pageSize = pagination?.pageSize ?? 25;
    const limit = pageSize;
    const offset = (current - 1) * pageSize;
    const qp = new URLSearchParams({
      ...(module ? { module } : {}),
      ...(type ? { type } : {}),
      ...filtersToParams(filters),
      limit: String(limit),
      offset: String(offset),
    });
    const { ok, data, error } = await apiCall('GET', `/api/entity?${qp.toString()}`);
    if (!ok) throw new Error(error || 'getList failed');
    const rows = data || [];
    // The backend has no COUNT(*) companion route, so total is exact only on
    // a non-full page; a full page reports "at least one more" via +1 so
    // Refine's pager still offers a next page instead of silently truncating.
    const total = rows.length < limit ? offset + rows.length : offset + rows.length + 1;
    return { data: rows, total };
  },

  getOne: async ({ resource, id }) => {
    const { ok, data, error } = await apiCall('GET', `/api/entity/${id}`);
    if (!ok) throw new Error(error || 'getOne failed');
    return { data };
  },

  create: async ({ resource, variables }) => {
    const { module, type } = splitResource(resource);
    const { ok, data, error } = await apiCall('POST', '/api/entity', {
      module,
      type,
      ...variables,
    });
    if (!ok) throw new Error(error || 'create failed');
    return { data };
  },

  update: async ({ id, variables }) => {
    const { ok, data, error } = await apiCall('PATCH', `/api/entity/${id}`, variables);
    if (!ok) throw new Error(error || 'update failed');
    return { data };
  },

  // The generic API deliberately has no DELETE route (events/entities are
  // lifecycle-managed via status, not hard-deleted) - this honestly throws
  // rather than pretending to succeed against a route that does not exist.
  deleteOne: async () => {
    throw new Error('deleteOne is not supported: entities are lifecycle-managed (status), not hard-deleted.');
  },
};
