const endpoint = import.meta.env.VITE_CHU_DEBUG_WS as string | undefined;

type DebugRequest = {
  id?: string;
  op: string;
  ref?: string;
  selector?: string;
  text?: string;
  value?: string;
  key?: string;
  x?: number;
  y?: number;
  script?: string;
  filename?: string;
};

type Handle = {
  ref: string;
  node: Element;
};

const handles = new Map<string, Element>();

function clean(value: string | null | undefined, limit = 160) {
  return (value || "").replace(/\s+/g, " ").trim().slice(0, limit);
}

function labelOf(node: Element) {
  if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) {
    return clean(node.getAttribute("aria-label") || node.labels?.[0]?.textContent || node.placeholder || node.name);
  }
  return clean(node.getAttribute("aria-label") || node.getAttribute("title") || node.textContent);
}

function roleOf(node: Element) {
  const explicit = node.getAttribute("role");
  if (explicit) return explicit;
  if (node instanceof HTMLButtonElement) return "button";
  if (node instanceof HTMLAnchorElement) return "link";
  if (node instanceof HTMLInputElement) return node.type === "checkbox" ? "checkbox" : "textbox";
  if (node instanceof HTMLTextAreaElement) return "textbox";
  if (node instanceof HTMLSelectElement) return "combobox";
  if (/^H[1-6]$/.test(node.tagName)) return "heading";
  return node.tagName.toLowerCase();
}

function interactive(node: Element) {
  if (node.getAttribute("aria-hidden") === "true") return false;
  if (node instanceof HTMLButtonElement || node instanceof HTMLAnchorElement) return true;
  if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement || node instanceof HTMLSelectElement) return true;
  return node.getAttribute("role") === "button";
}

function refreshHandles() {
  handles.clear();
  let index = 1;
  for (const node of document.querySelectorAll("h1,h2,h3,button,a,input,textarea,select,[role='button']")) {
    if (!interactive(node) && !/^H[1-6]$/.test(node.tagName)) continue;
    const rect = node.getBoundingClientRect();
    if (rect.width < 1 || rect.height < 1) continue;
    handles.set(`e${index}`, node);
    index += 1;
  }
}

function resolve(request: Pick<DebugRequest, "ref" | "selector">) {
  if (request.ref) {
    const node = handles.get(request.ref);
    if (!node?.isConnected) throw new Error(`stale ref: ${request.ref}; run snapshot`);
    return node;
  }
  if (!request.selector) throw new Error("missing ref or selector");
  const node = document.querySelector(request.selector);
  if (!node) throw new Error(`not found: ${request.selector}`);
  return node;
}

function boxOf(node: Element) {
  const rect = node.getBoundingClientRect();
  return {
    x: Math.round(rect.x),
    y: Math.round(rect.y),
    width: Math.round(rect.width),
    height: Math.round(rect.height),
  };
}

function nodeInfo(handle: Handle) {
  const node = handle.node;
  const input = node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement ? node : undefined;
  return {
    ref: handle.ref,
    role: roleOf(node),
    name: labelOf(node),
    value: input?.value,
    checked: node instanceof HTMLInputElement && (node.type === "checkbox" || node.type === "radio") ? node.checked : undefined,
    disabled: node instanceof HTMLButtonElement || node instanceof HTMLInputElement ? node.disabled : undefined,
    box: boxOf(node),
  };
}

function snapshot(selector?: string) {
  refreshHandles();
  const root = selector ? resolve({ selector }) : document.body;
  const visible = [...handles.entries()]
    .filter(([, node]) => root.contains(node))
    .map(([ref, node]) => nodeInfo({ ref, node }));
  return {
    url: location.href,
    title: document.title,
    viewport: { width: innerWidth, height: innerHeight },
    scrollHeight: document.scrollingElement?.scrollHeight ?? 0,
    text: clean(document.body.innerText, 2000),
    nodes: visible,
  };
}

function find(text?: string) {
  refreshHandles();
  const needle = (text || "").toLowerCase();
  return [...handles.entries()]
    .map(([ref, node]) => nodeInfo({ ref, node }))
    .filter((item) => `${item.role} ${item.name} ${item.value || ""}`.toLowerCase().includes(needle));
}

function click(node: Element) {
  if (node instanceof HTMLElement) node.click();
  else node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
}

function fill(node: Element, value: string) {
  if (!(node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement)) throw new Error("not an input");
  node.focus();
  node.value = value;
  node.dispatchEvent(new Event("input", { bubbles: true }));
  node.dispatchEvent(new Event("change", { bubbles: true }));
}

function press(key: string) {
  const targetNode = document.activeElement || document.body;
  targetNode.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
  targetNode.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true, cancelable: true }));
  if (key === "Enter" && targetNode instanceof HTMLElement) targetNode.click();
}

function scrollBy(x = 0, y = 0) {
  window.scrollBy(x, y);
  return { x: window.scrollX, y: window.scrollY };
}

function storage(kind: "local" | "session") {
  return kind === "session" ? sessionStorage : localStorage;
}

async function capture(request: DebugRequest) {
  const node = request.ref || request.selector ? resolve(request) : document.body;
  if (!(node instanceof HTMLElement)) throw new Error("not an element");
  const { snapdom } = await import("@zumer/snapdom");
  const rect = node.getBoundingClientRect();
  const canvas = await snapdom.toCanvas(node, {
    dpr: 1,
    scale: 1,
    width: Math.min(Math.ceil(rect.width), 1360),
    backgroundColor: "#fffdf4",
    embedFonts: false,
  });
  return { image: canvas.toDataURL("image/png"), width: canvas.width, height: canvas.height, filename: request.filename };
}

export async function runDebug(request: DebugRequest) {
  switch (request.op) {
    case "ping":
      return { ok: true, url: location.href };
    case "snapshot":
      return snapshot(request.selector);
    case "find":
      return { matches: find(request.text) };
    case "text":
      return { text: clean(resolve(request).textContent, 2000) };
    case "html":
      return { html: resolve(request).outerHTML.slice(0, 8000) };
    case "attr":
      return { value: resolve(request).getAttribute(request.key || "") };
    case "box":
      return boxOf(resolve(request));
    case "click":
    case "dblclick":
      click(resolve(request));
      if (request.op === "dblclick") click(resolve(request));
      return snapshot();
    case "hover":
      resolve(request).dispatchEvent(new MouseEvent("pointerover", { bubbles: true }));
      resolve(request).dispatchEvent(new MouseEvent("mouseover", { bubbles: true }));
      return { hovered: request.ref || request.selector };
    case "fill":
      fill(resolve(request), request.value || "");
      return snapshot();
    case "check":
    case "uncheck": {
      const node = resolve(request);
      if (!(node instanceof HTMLInputElement) || node.type !== "checkbox") throw new Error("not a checkbox");
      node.checked = request.op === "check";
      node.dispatchEvent(new Event("input", { bubbles: true }));
      node.dispatchEvent(new Event("change", { bubbles: true }));
      return snapshot();
    }
    case "select": {
      const node = resolve(request);
      if (!(node instanceof HTMLSelectElement)) throw new Error("not a select");
      node.value = request.value || "";
      node.dispatchEvent(new Event("input", { bubbles: true }));
      node.dispatchEvent(new Event("change", { bubbles: true }));
      return snapshot();
    }
    case "press":
      press(request.key || "Enter");
      return snapshot();
    case "type": {
      const active = document.activeElement;
      if (!(active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement)) throw new Error("no focused input");
      for (const char of request.text || "") {
        press(char);
        fill(active, `${active.value}${char}`);
      }
      return snapshot();
    }
    case "scroll":
      return scrollBy(request.x || 0, request.y || 0);
    case "mousemove":
      document.elementFromPoint(request.x || 0, request.y || 0)?.dispatchEvent(new MouseEvent("mousemove", { clientX: request.x, clientY: request.y, bubbles: true }));
      return { x: request.x, y: request.y };
    case "goto":
      history.pushState({}, "", request.value || "/");
      window.dispatchEvent(new PopStateEvent("popstate"));
      return snapshot();
    case "reload":
      location.reload();
      return { reloading: true };
    case "back":
      history.back();
      return { url: location.href };
    case "forward":
      history.forward();
      return { url: location.href };
    case "eval":
      return { result: await Function(`"use strict"; ${request.script}`)() };
    case "console":
      return { entries: consoleBuffer.slice(-50) };
    case "screenshot":
      return capture(request);
    case "localstorage-list":
      return { items: Object.fromEntries(Object.keys(localStorage).map((key) => [key, localStorage.getItem(key)])) };
    case "localstorage-get":
      return { value: storage("local").getItem(request.key || "") };
    case "localstorage-set":
      storage("local").setItem(request.key || "", request.value || "");
      return { key: request.key };
    case "localstorage-delete":
      storage("local").removeItem(request.key || "");
      return { key: request.key };
    case "localstorage-clear":
      storage("local").clear();
      return { cleared: true };
    case "sessionstorage-list":
      return { items: Object.fromEntries(Object.keys(sessionStorage).map((key) => [key, sessionStorage.getItem(key)])) };
    case "sessionstorage-get":
      return { value: storage("session").getItem(request.key || "") };
    case "sessionstorage-set":
      storage("session").setItem(request.key || "", request.value || "");
      return { key: request.key };
    case "sessionstorage-delete":
      storage("session").removeItem(request.key || "");
      return { key: request.key };
    case "sessionstorage-clear":
      storage("session").clear();
      return { cleared: true };
    default:
      throw new Error(`unknown op: ${request.op}`);
  }
}

const consoleBuffer: { level: string; message: string }[] = [];

function remember(level: string, args: unknown[]) {
  consoleBuffer.push({ level, message: args.map(String).join(" ").slice(0, 500) });
  if (consoleBuffer.length > 200) consoleBuffer.shift();
}

export function startDebugBridge() {
  if (!endpoint) return;
  for (const level of ["debug", "info", "warn", "error"] as const) {
    const original = console[level].bind(console);
    console[level] = (...args: unknown[]) => {
      remember(level, args);
      original(...args);
    };
  }
  window.addEventListener("error", (event) => remember("error", [event.message]));
  window.addEventListener("unhandledrejection", (event) => remember("error", [event.reason]));

  let socket: WebSocket | undefined;
  let retry = 500;

  function connect() {
    if (!endpoint) return;
    socket = new WebSocket(endpoint);
    socket.addEventListener("open", () => {
      retry = 500;
      socket?.send(JSON.stringify({ type: "hello", url: location.href }));
    });
    socket.addEventListener("message", async (event) => {
      const request = JSON.parse(String(event.data)) as DebugRequest;
      try {
        socket?.send(JSON.stringify({ id: request.id, ok: true, data: await runDebug(request) }));
      } catch (error) {
        socket?.send(JSON.stringify({ id: request.id, ok: false, error: String(error) }));
      }
    });
    socket.addEventListener("close", () => {
      window.setTimeout(connect, retry);
      retry = Math.min(retry * 2, 5000);
    });
  }

  connect();
}
