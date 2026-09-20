import { getCurrentWindow } from '@tauri-apps/api/window';

const WS = 'ws://127.0.0.1:44567';
let socket;
let currentKeys = ''; // serialized widget key list for structural diff

const bar = document.getElementById('bar');

// ── Send ───────────────────────────────────────────────────────────────────

function send(type, payload = {}) {
    if (socket?.readyState === WebSocket.OPEN)
        socket.send(JSON.stringify({ type, payload }));
}

function action(name, value = null) {
    send('action', { name, value });
}

// ── Window Resizing ────────────────────────────────────────────────────────

let resizeTimer = null;

async function updateWindowSize() {
    if (resizeTimer) cancelAnimationFrame(resizeTimer);
    resizeTimer = requestAnimationFrame(async () => {
        try {
            const barRect = bar.getBoundingClientRect();
            let width = Math.ceil(barRect.width);
            let height = Math.ceil(barRect.height);
            let extraUp = 0; // extra pixels the window must grow upward

            // Check dropdown
            const activeDropdown = bar.querySelector('.menu-dropdown.open');
            if (activeDropdown) {
                const ddRect = activeDropdown.getBoundingClientRect();
                const totalBottom = ddRect.bottom - barRect.top;
                const totalRight = ddRect.right - barRect.left;
                if (totalBottom > height) {
                    height = Math.ceil(totalBottom) + 10;
                }
                if (totalRight > width) {
                    width = Math.ceil(totalRight) + 10;
                }

                // Check visible sub-dropdowns (which are siblings of activeDropdown under .menu-widget)
                const activeSubs = bar.querySelectorAll('.menu-sub-dropdown.open');
                activeSubs.forEach(sub => {
                    const subRect = sub.getBoundingClientRect();
                    if (subRect.width > 0 && subRect.height > 0) {
                        const subBottom = subRect.bottom - barRect.top;
                        const subRight = subRect.right - barRect.left;
                        if (subBottom > height) height = Math.ceil(subBottom) + 10;
                        if (subRight > width) width = Math.ceil(subRight) + 10;
                    }
                });
            }

            // Check focused expanded input
            const activeInput = bar.querySelector('[data-wtype="input"]:focus');
            if (activeInput) {
                const inputRect = activeInput.getBoundingClientRect();
                const totalBottom = inputRect.bottom - barRect.top;
                if (totalBottom > height) {
                    height = Math.ceil(totalBottom) + 10;
                }
            }

            if (width > 0 && height > 0) {
                const { LogicalSize, LogicalPosition } = await import('@tauri-apps/api/dpi');
                const win = getCurrentWindow();
                const factor = await win.scaleFactor();
                const pos = await win.outerPosition();
                const logicalX = pos.x / factor;
                const logicalY = pos.y / factor;

                if (extraUp > 0) {
                    // Grow upward: record original Y once per expand session, then move up
                    if (!bar.dataset.winOrigY) {
                        bar.dataset.winOrigY = String(Math.round(logicalY));
                    }
                    const origY = parseInt(bar.dataset.winOrigY);
                    await win.setPosition(new LogicalPosition(logicalX, origY - extraUp));
                    await win.setSize(new LogicalSize(width, height + extraUp));
                } else {
                    // Normal: restore window Y if we previously moved it up
                    if (bar.dataset.winOrigY) {
                        const origY = parseInt(bar.dataset.winOrigY);
                        await win.setPosition(new LogicalPosition(logicalX, origY));
                        delete bar.dataset.winOrigY;
                    }
                    await win.setSize(new LogicalSize(width, height));
                }
            }
        } catch (e) { }
    });
}

function adjustTextareaHeight(el, expanded = false) {
    if (!expanded && document.activeElement !== el) {
        el.style.height = '26px';
        updateWindowSize();
        return;
    }
    el.style.height = 'auto';
    const newH = Math.min(Math.max(el.scrollHeight, 26), 140);
    el.style.height = `${newH}px`;
    updateWindowSize();
}

// ── Widget rendering ───────────────────────────────────────────────────────

function widgetKey(w) {
    return `${w.type}:${w.name || ''}`;
}

function buildDropdown(name, options, isSub = false) {
    const dropdown = document.createElement('div');
    dropdown.className = isSub ? 'menu-dropdown menu-sub-dropdown' : 'menu-dropdown';
    dropdown.addEventListener('mousedown', e => e.preventDefault());

    options.forEach(opt => {
        const item = document.createElement('div');
        const hasChildren = Array.isArray(opt.children) && opt.children.length > 0;
        item.className = 'menu-item' +
            (opt.checked ? ' checked' : '') +
            (opt.disabled ? ' disabled' : '') +
            (hasChildren ? ' has-children' : '');
        const labelEl = document.createElement('div');
        labelEl.className = 'menu-item-label';
        labelEl.textContent = opt.label;
        item.appendChild(labelEl);

        if (hasChildren) {
            const subDropdown = buildDropdown('model', opt.children, true);
            dropdown.appendChild(item);

            // Submenu positioning & hover logic outside the scroll container
            item.addEventListener('mouseenter', () => {
                const rect = item.getBoundingClientRect();
                const parentRect = dropdown.getBoundingClientRect();
                const wrapperRect = dropdown.parentElement ? dropdown.parentElement.getBoundingClientRect() : bar.getBoundingClientRect();
                subDropdown.style.top = `${rect.top - wrapperRect.top - 5}px`;
                subDropdown.style.left = `${parentRect.right - wrapperRect.left}px`;
                subDropdown.classList.add('open');
                updateWindowSize();
            });

            item.addEventListener('mouseleave', (e) => {
                const to = e.relatedTarget;
                if (!subDropdown.contains(to)) {
                    subDropdown.classList.remove('open');
                    updateWindowSize();
                }
            });

            subDropdown.addEventListener('mouseleave', (e) => {
                const to = e.relatedTarget;
                if (!item.contains(to)) {
                    subDropdown.classList.remove('open');
                    updateWindowSize();
                }
            });

            dropdown._pendingSubDropdowns = dropdown._pendingSubDropdowns || [];
            dropdown._pendingSubDropdowns.push(subDropdown);
            return;
        }

        item.addEventListener('click', e => {
            e.stopPropagation();
            if (opt.disabled) return;
            closeAllDropdowns();
            action(name, opt.value);
        });

        dropdown.appendChild(item);
    });
    return dropdown;
}

function makeWidget(w) {
    switch (w.type) {

        case 'drag': {
            const el = document.createElement('div');
            el.className = 'drag-handle';
            el.setAttribute('data-tauri-drag-region', '');
            el.textContent = '⠿';
            el.addEventListener('mousedown', e => e.preventDefault());
            return el;
        }

        case 'input': {
            const wrapper = document.createElement('div');
            wrapper.className = 'prompt-widget';
            wrapper.dataset.wkey = widgetKey(w);

            const el = document.createElement('textarea');
            el.rows = 1;
            el.dataset.wtype = 'input';
            el.dataset.wname = w.name;
            el.placeholder = w.placeholder || '';
            el.value = w.value || '';
            el.autocomplete = 'off';

            el.addEventListener('focus', () => {
                adjustTextareaHeight(el, true);
            });

            el.addEventListener('input', () => {
                adjustTextareaHeight(el, true);
            });

            el.addEventListener('blur', () => {
                adjustTextareaHeight(el, false);
                action(w.name, el.value);
            });

            el.addEventListener('change', () => action(w.name, el.value));

            el.addEventListener('keydown', (e) => {
                if (e.key === 'Escape') {
                    e.preventDefault();
                    el.blur();
                }
            });

            wrapper.appendChild(el);
            return wrapper;
        }

        case 'button': {
            const el = document.createElement('button');
            el.dataset.wtype = 'button';
            el.dataset.wname = w.name;
            el.hidden = !!w.hidden;
            el.disabled = !!w.disabled;
            if (w.title) el.title = w.title;

            let classes = [];
            if (w.variant) classes.push(w.variant);
            if (w.icon) classes.push('icon-btn', `icon-${w.icon}`);
            if (w.active) classes.push('active');
            if (classes.length) el.className = classes.join(' ');

            if (w.icon === 'crop-selection') {
                el.innerHTML = `<svg viewBox="0 0 56 56" class="btn-svg" width="14" height="14" fill="none" stroke="currentColor" stroke-width="4.5" stroke-linecap="round" stroke-linejoin="round"><path stroke-dasharray="7 5" d="M12 5v38h38"/><path stroke-dasharray="7 5" d="M44 51V13H6"/><line x1="12" y1="0" x2="12" y2="5"/><line x1="51" y1="43" x2="56" y2="43"/><line x1="0" y1="13" x2="5" y2="13"/><line x1="44" y1="51" x2="44" y2="56"/></svg>`;
            } else if (w.label) {
                el.textContent = w.label;
            }

            // Prevent button click from stealing focus away from Photoshop
            el.addEventListener('mousedown', e => e.preventDefault());
            el.addEventListener('click', () => {
                if (w.name === 'run') {
                    const activeInput = bar.querySelector('[data-wtype="input"][data-wname="prompt"]');
                    if (activeInput) {
                        action('prompt', activeInput.value);
                        activeInput.blur();
                    }
                }
                action(w.name);
            });
            return el;
        }

        case 'menu': {
            const wrapper = document.createElement('div');
            wrapper.className = 'menu-widget';
            wrapper.dataset.wkey = widgetKey(w);

            const btn = document.createElement('button');
            btn.dataset.wtype = 'menu';
            btn.dataset.wname = w.name;
            btn.textContent = w.label;
            // Prevent menu button from stealing focus
            btn.addEventListener('mousedown', e => e.preventDefault());

            const dropdown = buildDropdown(w.name, w.options || []);

            btn.addEventListener('click', e => {
                e.stopPropagation();
                const currentDd = wrapper.querySelector('.menu-dropdown');
                const isOpen = currentDd?.classList.contains('open');
                closeAllDropdowns();
                if (!isOpen && currentDd) {
                    currentDd.classList.add('open');
                }
                updateWindowSize();
            });

            wrapper.appendChild(btn);
            wrapper.appendChild(dropdown);
            if (dropdown._pendingSubDropdowns) {
                dropdown._pendingSubDropdowns.forEach(sub => wrapper.appendChild(sub));
            }
            return wrapper;
        }

        case 'divider': {
            const el = document.createElement('div');
            el.className = 'divider';
            return el;
        }
    }
    return null;
}

function patchWidget(el, w) {
    // Update an existing element in-place (preserves focus)
    switch (w.type) {
        case 'input': {
            // Prompt input is local state in the Turbo Bar; never overwrite from server.
            const textarea = el.querySelector('[data-wtype="input"]') || el;
            textarea.placeholder = w.placeholder || '';
            break;
        }
        case 'button':
            if (w.title) el.title = w.title;
            el.hidden = !!w.hidden;
            el.disabled = !!w.disabled;
            el.classList.toggle('active', !!w.active);
            if (w.variant) {
                if (!el.classList.contains(w.variant)) el.classList.add(w.variant);
            }
            if (!w.icon && w.label !== undefined) {
                el.textContent = w.label;
            }
            break;
        case 'menu': {
            // Patch dropdown options (button label stays same: '•••')
            const dropdown = el.querySelector('.menu-dropdown');
            if (!dropdown) break;
            el.querySelectorAll('.menu-sub-dropdown').forEach(s => s.remove());
            const newDd = buildDropdown(w.name, w.options || []);
            dropdown.replaceWith(newDd);
            if (newDd._pendingSubDropdowns) {
                newDd._pendingSubDropdowns.forEach(sub => el.appendChild(sub));
            }
            break;
        }
    }
}

function renderWidgets(widgets) {
    const newKeys = widgets.map(widgetKey).join(',');

    if (newKeys !== currentKeys) {
        // Structure changed: full rebuild
        bar.innerHTML = '';
        widgets.forEach(w => {
            const el = makeWidget(w);
            if (el) bar.appendChild(el);
        });
        currentKeys = newKeys;
    } else {
        // Same structure: patch in place
        let elIndex = 0;
        widgets.forEach(w => {
            const el = bar.children[elIndex++];
            if (el) patchWidget(w.type === 'menu' ? el : el, w);
        });
    }

    // Adapt window size to content
    updateWindowSize();
}

// ── Dropdown close ─────────────────────────────────────────────────────────

function closeAllDropdowns() {
    let closedAny = false;
    bar.querySelectorAll('.menu-dropdown.open').forEach(d => {
        d.classList.remove('open');
        closedAny = true;
    });
    if (closedAny) {
        updateWindowSize();
    }
}

document.addEventListener('click', closeAllDropdowns);

// Clicking inside window outside the prompt widget: blur textarea
document.addEventListener('mousedown', (e) => {
    const activeTextarea = document.activeElement;
    if (activeTextarea?.dataset?.wtype === 'input' && !e.target.closest('.prompt-widget')) {
        activeTextarea.blur();
    }
});

// Clicking outside the Tauri window (window loses focus): blur textarea
window.addEventListener('blur', () => {
    const activeTextarea = document.activeElement;
    if (activeTextarea?.dataset?.wtype === 'input') {
        activeTextarea.blur();
    }
});

// ── Receive ────────────────────────────────────────────────────────────────

let _isShowing = false;

async function showBar() {
    if (_isShowing) return;
    _isShowing = true;
    try {
        const win = getCurrentWindow();
        await win.show();
        // Re-assert alwaysOnTop: macOS may drop it after the window goes background
        await win.setAlwaysOnTop(true);
    } catch (e) { }
    _isShowing = false;
}

function receive({ type, payload = {} }) {
    if (type === 'state') {
        if (payload.mode === 'turbo') {
            showBar();
            if (Array.isArray(payload.widgets)) renderWidgets(payload.widgets);
        } else {
            getCurrentWindow().hide();
        }
        return;
    }
    if (type === 'close') {
        getCurrentWindow().hide();
    }
}

// ── Connection ─────────────────────────────────────────────────────────────

function connect() {
    socket = new WebSocket(WS);
    socket.onopen = () => send('ready');
    socket.onmessage = event => {
        try { receive(JSON.parse(event.data)); } catch (e) { }
    };
    socket.onclose = () => { socket = null; setTimeout(connect, 1000); };
}

connect();
