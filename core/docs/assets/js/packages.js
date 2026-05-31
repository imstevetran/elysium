/**
 * Package registry UI — loads packages from the EPM registry raw index.
 */
(function () {
  const REGISTRY_URL =
    'https://raw.githubusercontent.com/imstevetran/epm-registry/main/registry.json';
  const META_URL = '../data/packages-meta.json';

  const root = document.getElementById('packages-registry');
  if (!root) return;

  function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text ?? '';
    return div.innerHTML;
  }

  function renderPackage(pkg) {
    const tags = (pkg.tags || [])
      .map((t) => `<span class="card-tag">${escapeHtml(t)}</span>`)
      .join('');
    const inner = `
      <span class="icon">${escapeHtml(pkg.icon || '📦')}</span>
      <h3>${escapeHtml(pkg.name)} <span class="badge">${escapeHtml(pkg.version)}</span></h3>
      <p>${escapeHtml(pkg.description || '')}</p>
      <div class="card-meta">${tags}</div>
    `;
    if (pkg.href) {
      return `<a href="${escapeHtml(pkg.href)}" class="feature-card">${inner}</a>`;
    }
    return `<div class="feature-card feature-card-static">${inner}</div>`;
  }

  function renderCategory(section) {
    const cards = section.packages.map(renderPackage).join('');
    return `
      <section class="animate-on-scroll" style="margin-top: ${section.order === 1 ? '32' : '48'}px;">
        <h2 class="section-title">${escapeHtml(section.title)}</h2>
        <p class="section-subtitle">${escapeHtml(section.subtitle)}</p>
        <div class="features" style="margin: 0;">${cards}</div>
      </section>
    `;
  }

  function showError(message) {
    root.innerHTML = `
      <section class="content-section">
        <p class="text-dim">${escapeHtml(message)}</p>
        <p>Registry: <a href="${escapeHtml(REGISTRY_URL)}" target="_blank" rel="noopener">${escapeHtml(REGISTRY_URL)}</a></p>
      </section>
    `;
  }

  function normalizePackages(registry, meta) {
    const categories = meta.categories || {};
    const pkgMeta = meta.packages || {};
    const entries = Object.values(registry.packages || {});

    const items = entries.map((entry) => {
      const name = entry.name;
      const pm = pkgMeta[name] || {};
      const catId = pm.category || 'other';
      const cat = categories[catId] || categories.other || { title: 'Packages', subtitle: '', order: 99 };
      return {
        name,
        version: (entry.versions && entry.versions[0]) || '0.0.0',
        description: entry.description || '',
        icon: pm.icon || '📦',
        tags: pm.tags || [],
        category: catId,
        categoryTitle: cat.title,
        categorySubtitle: cat.subtitle,
        categoryOrder: cat.order ?? 99,
        href: pm.docSlug ? `${pm.docSlug}/` : null,
      };
    });

    items.sort((a, b) => a.categoryOrder - b.categoryOrder || a.name.localeCompare(b.name));

    const byCategory = new Map();
    for (const item of items) {
      if (!byCategory.has(item.category)) {
        byCategory.set(item.category, {
          id: item.category,
          title: item.categoryTitle,
          subtitle: item.categorySubtitle,
          order: item.categoryOrder,
          packages: [],
        });
      }
      const { categoryTitle, categorySubtitle, categoryOrder, category, ...pkg } = item;
      byCategory.get(item.category).packages.push(pkg);
    }

    return [...byCategory.values()].sort((a, b) => a.order - b.order);
  }

  function observeAnimations() {
    document.querySelectorAll('#packages-registry .animate-on-scroll').forEach((el) => {
      if (typeof IntersectionObserver === 'undefined') {
        el.classList.add('animate');
        return;
      }
      const observer = new IntersectionObserver(
        (entries) => {
          entries.forEach((entry) => {
            if (entry.isIntersecting) {
              entry.target.classList.add('animate');
              observer.unobserve(entry.target);
            }
          });
        },
        { threshold: 0.1 }
      );
      observer.observe(el);
    });
  }

  function updateMeta(count) {
    const el = document.getElementById('packages-index-meta');
    if (el) {
      el.textContent = `${count} package${count === 1 ? '' : 's'} from EPM registry`;
    }
  }

  async function load() {
    try {
      const [registryRes, metaRes] = await Promise.all([
        fetch(REGISTRY_URL, { cache: 'no-cache' }),
        fetch(META_URL, { cache: 'no-cache' }),
      ]);
      if (!registryRes.ok) throw new Error(`registry HTTP ${registryRes.status}`);
      const registry = await registryRes.json();
      const meta = metaRes.ok ? await metaRes.json() : { categories: {}, packages: {} };

      const categories = normalizePackages(registry, meta);
      const count = categories.reduce((n, c) => n + c.packages.length, 0);

      if (count === 0) {
        showError('No packages published in the registry yet.');
        return;
      }

      root.innerHTML = categories.map(renderCategory).join('');
      updateMeta(count);
      observeAnimations();
    } catch (err) {
      showError(`Could not load registry: ${err.message}`);
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', load);
  } else {
    load();
  }
})();
