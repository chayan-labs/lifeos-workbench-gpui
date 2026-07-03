// Life OS service worker (issue #103, docs/PLATFORM-SYSTEMS.md).
// Caches the app shell so the SPA loads offline, and runtime-caches GET
// /api/* responses (stale-while-revalidate) so the embedded-replica read
// model stays usable offline - reads only, matching the project's "reads
// are free, writes are gated" posture (docs/SECURITY.md §1).

const SHELL_CACHE = 'lifeos-shell-v1';
const API_CACHE = 'lifeos-api-v1';
const SHELL_URLS = ['/', '/dashboard', '/manifest.webmanifest', '/icon.svg', '/icon-192.png', '/icon-512.png', '/apple-touch-icon.png'];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(SHELL_CACHE).then((cache) => cache.addAll(SHELL_URLS)).then(() => self.skipWaiting())
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k !== SHELL_CACHE && k !== API_CACHE).map((k) => caches.delete(k))))
      .then(() => self.clients.claim())
  );
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  if (request.method !== 'GET') return; // writes are never cached/served from cache

  const url = new URL(request.url);

  if (url.pathname.startsWith('/api/')) {
    // Stale-while-revalidate: serve cached instantly, refresh in the background.
    event.respondWith(
      caches.open(API_CACHE).then(async (cache) => {
        const cached = await cache.match(request);
        const fetchPromise = fetch(request)
          .then((response) => {
            if (response.ok) cache.put(request, response.clone());
            return response;
          })
          .catch(() => cached);
        return cached || fetchPromise;
      })
    );
    return;
  }

  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).catch(() => caches.match('/dashboard').then((r) => r || caches.match('/')))
    );
    return;
  }

  event.respondWith(caches.match(request).then((cached) => cached || fetch(request)));
});

self.addEventListener('push', (event) => {
  const data = event.data ? event.data.json() : { title: 'Life OS', body: 'New activity.' };
  event.waitUntil(
    self.registration.showNotification(data.title || 'Life OS', {
      body: data.body || '',
      icon: '/icon-192.png',
      // Mirrors the Telegram digest's alert-red gating (docs/PLATFORM-SYSTEMS.md) -
      // the payload carries the same urgency tag the digest uses.
      tag: data.tag || 'lifeos-digest',
    })
  );
});
