/* Service Worker — PWA Offline para UCI.
 * Estrategia:
 *  - Navegaciones: red primero, caché como respaldo (app shell accesible offline).
 *  - Estáticos (WASM, CSS, iconos, manifest): cache-first con revalidación en
 *    segundo plano (stale-while-revalidate).
 *  - NO cachear /api ni /obs: los datos clínicos siempre se consultan frescos.
 */
const CACHE_VERSION = 'uci-v1';
const PRECACHE_URLS = [
  '/',
  '/index.html',
  '/manifest.webmanifest',
  '/icon.svg',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches.open(CACHE_VERSION).then((cache) => {
      return Promise.allSettled(PRECACHE_URLS.map((u) => cache.add(u)));
    })
  );
  self.skipWaiting();
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches.keys().then((keys) => {
      return Promise.all(
        keys.filter((k) => k !== CACHE_VERSION).map((k) => caches.delete(k))
      );
    })
  );
  self.clients.claim();
});

self.addEventListener('fetch', (event) => {
  const { request } = event;
  const url = new URL(request.url);

  // Solo GET, mismo origen y fuera del API/obs.
  if (
    request.method !== 'GET' ||
    url.origin !== self.location.origin ||
    url.pathname.startsWith('/api') ||
    url.pathname.startsWith('/obs')
  ) {
    return;
  }

  // Navegaciones: red primero, caché como respaldo.
  if (request.mode === 'navigate') {
    event.respondWith(
      fetch(request).then((response) => {
        const copy = response.clone();
        caches.open(CACHE_VERSION).then((cache) => cache.put(request, copy));
        return response;
      }).catch(() => {
        return caches.match(request).then((cached) => {
          return cached || caches.match('/index.html');
        });
      })
    );
    return;
  }

  // Estáticos (WASM, CSS, fuentes, manifest): cache-first + revalidación en segundo plano.
  event.respondWith(
    caches.open(CACHE_VERSION).then((cache) => {
      return cache.match(request).then((cached) => {
        const networkFetch = fetch(request).then((response) => {
          if (response && response.status === 200) {
            const copy = response.clone();
            cache.put(request, copy);
          }
          return response;
        }).catch(() => cached);
        return cached || networkFetch;
      });
    })
  );
});