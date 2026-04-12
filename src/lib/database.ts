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
  return db.select<T>(query, params);
}

export async function execute(query: string, params?: unknown[]): Promise<void> {
  const db = await getDb();
  await db.execute(query, params);
}
