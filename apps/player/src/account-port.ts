import { invoke } from "@tauri-apps/api/core";
import type { Account, AccountPort, Character, Item, Skill, Stat } from "./contracts";

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function integer(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value);
}

function stat(value: unknown): value is Stat {
  return Array.isArray(value) && value.length === 2 &&
    typeof value[0] === "string" && typeof value[1] === "string";
}

function item(value: unknown): value is Item {
  return record(value) && typeof value.name === "string" &&
    typeof value.image === "string" && typeof value.detail === "string" && integer(value.count);
}

function skill(value: unknown): value is Skill {
  return record(value) && typeof value.group === "string" && typeof value.tab === "string" &&
    (value.title === null || typeof value.title === "string") &&
    (value.level === null || integer(value.level)) && typeof value.description === "string" &&
    Array.isArray(value.scaling) && value.scaling.every(stat);
}

function character(value: unknown): value is Character {
  return record(value) && typeof value.id === "string" && typeof value.name === "string" &&
    typeof value.path === "string" && typeof value.image === "string" &&
    (value.portrait === undefined || typeof value.portrait === "string") &&
    (value.level === null || integer(value.level)) &&
    (value.ascension === null || integer(value.ascension)) &&
    (value.eidolons === null || integer(value.eidolons)) &&
    Array.isArray(value.stats) && value.stats.every(stat) &&
    Array.isArray(value.skills) && value.skills.every(skill) &&
    (value.cone === undefined || item(value.cone)) &&
    Array.isArray(value.relics) && value.relics.every(item);
}

function account(value: unknown): value is Account {
  return record(value) && typeof value.id === "string" && typeof value.name === "string" &&
    typeof value.source === "string" && Array.isArray(value.characters) &&
    value.characters.every(character);
}

async function openAccount(command: string, args: Record<string, unknown>): Promise<Account> {
  const result: unknown = await invoke(command, args);
  if (!account(result)) throw new Error("账号服务返回了无法读取的角色档案。");
  return result;
}

export function desktopAccounts(preview: AccountPort): AccountPort {
  return {
    async accounts() {
      const result: unknown = await invoke("player_accounts");
      if (!Array.isArray(result) || !result.every(account)) {
        throw new Error("账号服务返回了无法读取的账号列表。");
      }
      return [...result, ...await preview.accounts()];
    },
    importAccount(fileName, json) {
      return openAccount("player_import_account", { fileName, json });
    },
    generateAccount(seed) {
      return openAccount("player_generate_account", { seed });
    },
  };
}
