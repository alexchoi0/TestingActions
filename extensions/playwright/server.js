/**
 * Playwright Bridge Server
 *
 * JSON-RPC server that wraps Playwright for browser automation.
 * Communicates via stdin/stdout with newline-delimited JSON.
 */

const readline = require('readline');
const { chromium, firefox, webkit } = require('playwright');

// Storage for browser instances and pages
const browsers = new Map();
const pages = new Map();

// JSON-RPC helpers
function success(id, result) {
    console.log(JSON.stringify({ jsonrpc: '2.0', id, result }));
}

function error(id, code, message) {
    console.log(JSON.stringify({ jsonrpc: '2.0', id, error: { code, message } }));
}

// Get browser launcher by type
function getBrowserType(type) {
    switch (type) {
        case 'firefox': return firefox;
        case 'webkit': return webkit;
        case 'chromium':
        default: return chromium;
    }
}

// Method handlers
const methods = {
    // Browser management
    'browser.launch': async ({ browserType, headless }) => {
        const launcher = getBrowserType(browserType);
        const browser = await launcher.launch({ headless: headless !== false });
        const id = `browser_${Date.now()}_${Math.random().toString(36).slice(2)}`;
        browsers.set(id, browser);
        return { browserId: id };
    },

    'browser.close': async ({ browserId }) => {
        const browser = browsers.get(browserId);
        if (browser) {
            await browser.close();
            browsers.delete(browserId);
            // Clean up pages for this browser
            for (const [pageId, page] of pages) {
                if (page._browserId === browserId) {
                    pages.delete(pageId);
                }
            }
        }
        return {};
    },

    // Page management
    'page.new': async ({ browserId }) => {
        const browser = browsers.get(browserId);
        if (!browser) throw new Error(`Browser not found: ${browserId}`);

        const context = await browser.newContext();
        const page = await context.newPage();
        const id = `page_${Date.now()}_${Math.random().toString(36).slice(2)}`;
        page._browserId = browserId;
        pages.set(id, page);
        return { pageId: id };
    },

    'page.close': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (page) {
            await page.close();
            pages.delete(pageId);
        }
        return {};
    },

    'page.goto': async ({ pageId, url }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.goto(url);
        return {};
    },

    'page.reload': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.reload();
        return {};
    },

    'page.goBack': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.goBack();
        return {};
    },

    'page.goForward': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.goForward();
        return {};
    },

    'page.url': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        return { url: page.url() };
    },

    'page.title': async ({ pageId }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        return { title: await page.title() };
    },

    'page.screenshot': async ({ pageId, path, fullPage }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.screenshot({ path, fullPage: fullPage === true });
        return {};
    },

    'page.pdf': async ({ pageId, path }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.pdf({ path });
        return {};
    },

    // Element interactions
    'element.click': async ({ pageId, selector }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.click(selector);
        return {};
    },

    'element.fill': async ({ pageId, selector, value }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.fill(selector, value);
        return {};
    },

    'element.type': async ({ pageId, selector, text, delay }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.type(selector, text, { delay: delay || 0 });
        return {};
    },

    'element.select': async ({ pageId, selector, value }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.selectOption(selector, value);
        return {};
    },

    'element.hover': async ({ pageId, selector }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.hover(selector);
        return {};
    },

    'element.textContent': async ({ pageId, selector }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        const text = await page.textContent(selector);
        return { text: text || '' };
    },

    'element.getAttribute': async ({ pageId, selector, attribute }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        const value = await page.getAttribute(selector, attribute);
        return { value };
    },

    'element.isVisible': async ({ pageId, selector }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        const visible = await page.isVisible(selector);
        return { visible };
    },

    // Wait actions
    'wait.selector': async ({ pageId, selector, timeout }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.waitForSelector(selector, { timeout: timeout || 30000 });
        return {};
    },

    'wait.navigation': async ({ pageId, timeout }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.waitForNavigation({ timeout: timeout || 30000 });
        return {};
    },

    'wait.url': async ({ pageId, pattern, timeout }) => {
        const page = pages.get(pageId);
        if (!page) throw new Error(`Page not found: ${pageId}`);
        await page.waitForURL(pattern, { timeout: timeout || 30000 });
        return {};
    },
};

// Main JSON-RPC loop
const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    terminal: false,
});

rl.on('line', async (line) => {
    let request;
    try {
        request = JSON.parse(line);
    } catch (e) {
        console.error('Invalid JSON:', line);
        return;
    }

    const { id, method, params } = request;

    const handler = methods[method];
    if (!handler) {
        error(id, -32601, `Method not found: ${method}`);
        return;
    }

    try {
        const result = await handler(params || {});
        success(id, result);
    } catch (e) {
        error(id, -32000, e.message);
    }
});

// Cleanup on exit
async function cleanup() {
    for (const browser of browsers.values()) {
        try {
            await browser.close();
        } catch (e) {
            // Ignore cleanup errors
        }
    }
    process.exit(0);
}

process.on('SIGINT', cleanup);
process.on('SIGTERM', cleanup);
rl.on('close', cleanup);

console.error('Playwright bridge server started');
