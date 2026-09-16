/** Minimal glob matcher for protected-path patterns. No npm deps (Pi loads this via jiti). */

export function normalizePath(p: string): string {
  return p.replace(/\\/g, "/");
}

export function globToRegExp(glob: string): RegExp {
  const g = normalizePath(glob);
  let out = "";
  for (let i = 0; i < g.length; i++) {
    const c = g[i];
    if (c === "*" && g[i + 1] === "*") {
      if (g[i + 2] === "/") {
        out += "(?:.*/)?";
        i += 2;
      } else {
        out += ".*";
        i += 1;
      }
    } else if (c === "*") {
      out += "[^/]*";
    } else if (c === "?") {
      out += "[^/]";
    } else if ("+.^${}()|[]\\".includes(c)) {
      out += `\\${c}`;
    } else {
      out += c;
    }
  }
  return new RegExp(`^${out}$`, "i");
}

export function pathMatchesGlob(filePath: string, glob: string): boolean {
  const norm = normalizePath(filePath);
  const re = globToRegExp(glob);
  if (re.test(norm)) return true;
  if (norm.startsWith("/") && re.test(norm.slice(1))) return true;
  const parts = norm.split("/").filter(Boolean);
  for (let i = 0; i < parts.length; i++) {
    const suffix = parts.slice(i).join("/");
    if (re.test(suffix) || re.test("/" + suffix)) return true;
  }
  return false;
}

export function pathMatchesAny(filePath: string, globs: string[]): boolean {
  return globs.some((g) => pathMatchesGlob(filePath, g));
}
