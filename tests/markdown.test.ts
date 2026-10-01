import { describe, expect, it } from "vitest";
import { markdown, inline, safeLink } from "../src/lib/markdown";
describe("safe Markdown", () => {
  it("preserves fenced code and incomplete streaming fences", () => {
    const code = '<script>alert(1)</script>\nconst x = 2;';
    expect(markdown("# Title\n\n```js\n" + code)[1]).toEqual({ kind: "code", language: "js", text: code });
  });
  it("rejects active-content and malformed URLs", () => {
    for (const url of ["javascript:alert(1)", "data:text/html,test", "//evil.test", "java\nscript:test", "file:///etc/passwd"]) expect(safeLink(url)).toBeUndefined();
    expect(safeLink("https://example.com")).toBe("https://example.com/");
    expect(inline("[unsafe](javascript:alert)")[0].kind).toBe("text");
  });
  it("parses headings, lists, emphasis and links", () => {
    expect(markdown("# Title\n\n- one\n- two")[1]).toEqual({ kind: "list", ordered: false, items: ["one", "two"] });
    expect(inline("**bold** and `code`").map((token) => token.kind)).toEqual(["strong", "text", "code"]);
  });
});
