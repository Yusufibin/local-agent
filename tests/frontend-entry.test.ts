/**
 * @vitest-environment happy-dom
 */
import { afterEach, describe, expect, it } from "vitest";
import { tick } from "svelte";

describe("browser entry", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  it("executes without throw and exposes chat chrome", async () => {
    document.body.innerHTML = '<div id="app"></div>';
    await import("../src/main.ts");
    expect(document.querySelector('[data-testid="sessions"]')).toBeTruthy();
    expect(document.querySelector('[data-testid="transcript"]')).toBeTruthy();
    expect(document.querySelector('[data-testid="composer-input"]')).toBeTruthy();
    expect(document.querySelector('[data-testid="status"]')).toBeTruthy();
    const shell = document.querySelector('[data-testid="shell"]');
    const toggle = document.querySelector('[data-testid="chrome-toggle"]') as HTMLButtonElement | null;
    expect(shell).toBeTruthy();
    expect(toggle).toBeTruthy();
    expect(shell?.classList.contains("chrome-open")).toBe(false);
    expect(toggle?.getAttribute("aria-expanded")).toBe("false");
    toggle?.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    await tick();
    expect(shell?.classList.contains("chrome-open")).toBe(true);
    expect(toggle?.getAttribute("aria-expanded")).toBe("true");
  });
});