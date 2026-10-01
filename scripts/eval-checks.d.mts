export type AnswerCheck = { all?: string[]; any?: string[]; none?: string[] };
export function validateSuite(suite: unknown): void;
export function checkAnswer(check: AnswerCheck, answer: string): boolean;
export function scoreTask(task: unknown, answer: string, workspace: string, originals: Record<string, string>): { kind: string; passed: boolean; detail: string }[];
