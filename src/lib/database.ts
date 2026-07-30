import Database from "@tauri-apps/plugin-sql";

const DB_NAME = "sqlite:codex-ai.db";
let dbInstance: Database | null = null;

export async function getDb(): Promise<Database> {
  if (!dbInstance) {
    dbInstance = await Database.load(DB_NAME);
  }
  return dbInstance;
}

export async function select<T>(query: string, params?: unknown[]): Promise<T[]> {
  const db = await getDb();
  return db.select(query, params) as Promise<T[]>;
}

/**
 * @deprecated Frontend writes are blocked by capabilities (no sql:allow-execute).
 * Use Tauri commands (e.g. logActivity) for all mutations.
 */
export async function execute(_query: string, _params?: unknown[]): Promise<void> {
  throw new Error("前端禁止直接执行写 SQL。请改用后端 Tauri command（例如 logActivity）。");
}
