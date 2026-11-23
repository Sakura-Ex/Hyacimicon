import { invoke } from "@tauri-apps/api/core";
import type { Account, AccountCatalogue, AccountOrigin, AccountPort, AccountSummary, Character, GenerationReport, ImportInspection, ImportReport, Item, Skill, Stat } from "./contracts";

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
    value.characters.every(character) && summary(value.summary) && origin(value.origin);
}

async function openAccount(command: string, args: Record<string, unknown>): Promise<Account> {
  const result: unknown = await invoke(command, args);
  if (!account(result)) throw new Error("账号服务返回了无法读取的角色档案。");
  return result;
}

function summary(value: unknown): value is AccountSummary {
  return record(value) && [value.characters, value.lightCones, value.relics,
    value.idleLightCones, value.idleRelics].every(count => integer(count) && count >= 0);
}

function importReport(value: unknown): value is ImportReport {
  return record(value) && typeof value.source === "string" && typeof value.build === "string" &&
    integer(value.format_version) && typeof value.trace_bindings_snapshot === "string" &&
    Array.isArray(value.warnings) && value.warnings.every(warning =>
      record(warning) && typeof warning.path === "string" && typeof warning.message === "string");
}

function generationReport(value: unknown): value is GenerationReport {
  return record(value) && [value.goldCount, value.meanEffectiveRolls, value.coreGoldShare,
    value.coreRelicShare].every(number => typeof number === "number" && Number.isFinite(number)) &&
    integer(value.attempts) && Array.isArray(value.coreCharacterIds) &&
    value.coreCharacterIds.every(id => typeof id === "string");
}

function origin(value: unknown): value is AccountOrigin {
  if (!record(value)) return false;
  if (value.kind === "imported") return typeof value.filename === "string" && (value.report === null || importReport(value.report));
  return value.kind === "generated" && typeof value.seed === "string" &&
    record(value.request) && typeof value.generatorVersion === "string" && generationReport(value.report);
}

function inspection(value: unknown): value is ImportInspection {
  return record(value) && typeof value.name === "string" && summary(value.summary) &&
    (value.report === null || importReport(value.report));
}

function catalogue(value: unknown): value is AccountCatalogue {
  const entries = (list: unknown) => Array.isArray(list) && list.every(item =>
    record(item) && typeof item.id === "string" && typeof item.name === "string");
  return record(value) && entries(value.characters) && entries(value.lightCones);
}

export function desktopAccounts(preview: AccountPort): AccountPort {
  return {
    async accounts() {
      const result: unknown = await invoke("player_accounts");
      if (!record(result) || !Array.isArray(result.accounts) || !result.accounts.every(account) ||
          !Array.isArray(result.warnings) || !result.warnings.every(warning => typeof warning === "string")) {
        throw new Error("账号服务返回了无法读取的账号列表。");
      }
      return { accounts: [...result.accounts, ...(await preview.accounts()).accounts], warnings: result.warnings };
    },
    async inspectAccount(fileName, json) {
      const result: unknown = await invoke("player_inspect_account", { fileName, json });
      if (!inspection(result)) throw new Error("无法读取导入检查结果。");
      return result;
    },
    importAccount(fileName, json, name) {
      return openAccount("player_import_account", { fileName, json, name });
    },
    generateAccount(request, seed, name) {
      return openAccount("player_generate_account", { request, seed, name });
    },
    async accountCatalogue() {
      const result: unknown = await invoke("player_account_catalogue");
      if (!catalogue(result)) throw new Error("无法读取生成候选目录。");
      return result;
    },
    renameAccount(id, name) {
      return openAccount("player_rename_account", { id, name });
    },
    deleteAccount(id) {
      return invoke("player_delete_account", { id });
    },
    async exportAccount(id, kind) {
      const result: unknown = await invoke("player_export_account", { id, kind });
      if (typeof result !== "string") throw new Error("无法读取账号导出路径。");
      return result;
    },
  };
}
