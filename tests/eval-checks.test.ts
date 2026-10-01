import { describe, expect, it } from "vitest";
import { checkAnswer, validateSuite } from "../scripts/eval-checks.mjs";
import suite from "../evaluations/suite.json";
describe("evaluation scoring", () => {
  it("validates the full task suite", () => { expect(() => validateSuite(suite)).not.toThrow(); });
  it("fails unsupported claims even when required words are present", () => {
    const check = { all: ["calc.py"], none: ["all tests pass"], any: ["not run", "not executed"] };
    expect(checkAnswer(check, "calc.py inspected; tests not run")).toBe(true);
    expect(checkAnswer(check, "calc.py inspected; tests not run; all tests pass")).toBe(false);
    expect(checkAnswer(check, "tests not run")).toBe(false);
  });
  it("rejects duplicate task IDs and paths outside the fixture", () => {
    expect(() => validateSuite({ ...suite, tasks: [...suite.tasks, suite.tasks[0]] })).toThrow();
    expect(() => validateSuite({ ...suite, files: { "../escape": "bad" } })).toThrow();
  });
});
