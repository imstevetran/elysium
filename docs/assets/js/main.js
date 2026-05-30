// Mobile menu toggle
document.addEventListener('DOMContentLoaded', () => {
  const toggle = document.querySelector('.menu-toggle');
  const nav = document.querySelector('.navbar nav');
  if (toggle && nav) {
    toggle.addEventListener('click', () => {
      nav.classList.toggle('open');
    });
  }

  // Highlight current page in nav
  const currentPath = window.location.pathname.replace(/\/+$/, '') || '/index.html';
  document.querySelectorAll('.navbar nav a').forEach(link => {
    const href = link.getAttribute('href');
    if (href === currentPath || currentPath.endsWith(href)) {
      link.classList.add('active');
    }
  });

  // Animate elements on scroll
  const observer = new IntersectionObserver(
    (entries) => {
      entries.forEach(entry => {
        if (entry.isIntersecting) {
          entry.target.classList.add('animate');
          observer.unobserve(entry.target);
        }
      });
    },
    { threshold: 0.1 }
  );

  document.querySelectorAll('.animate-on-scroll').forEach(el => observer.observe(el));

  // Copy code blocks
  document.querySelectorAll('pre code').forEach(block => {
    const pre = block.parentElement;
    const btn = document.createElement('button');
    btn.textContent = 'Copy';
    btn.style.cssText = `
      position: absolute; top: 8px; right: 8px;
      background: var(--bg-card); border: 1px solid var(--border);
      color: var(--text-dim); padding: 4px 10px; border-radius: var(--radius-xs);
      font-size: 0.75rem; cursor: pointer; font-family: var(--font-sans);
      opacity: 0; transition: opacity 0.2s;
    `;
    pre.style.position = 'relative';
    pre.appendChild(btn);

    pre.addEventListener('mouseenter', () => btn.style.opacity = '1');
    pre.addEventListener('mouseleave', () => btn.style.opacity = '0');

    btn.addEventListener('click', async () => {
      try {
        await navigator.clipboard.writeText(block.textContent);
        btn.textContent = 'Copied!';
        btn.style.color = 'var(--green)';
        setTimeout(() => {
          btn.textContent = 'Copy';
          btn.style.color = 'var(--text-dim)';
        }, 2000);
      } catch {
        btn.textContent = 'Failed';
        setTimeout(() => { btn.textContent = 'Copy'; }, 2000);
      }
    });
  });
});
