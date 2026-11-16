"""从私有数据仓库的图鉴快照生成运营规则包。"""

import hashlib
import json
from dataclasses import replace
from pathlib import Path

from .catalog import RulePack, load_rule_pack
from .models import (
    NodeType,
    Position,
    RuleMetadata,
    TraitDefinition,
    TraitTier,
    UnitDefinition,
)

SNAPSHOT_FILES = (
    "characters",
    "traits",
    "trait_details",
    "presentation",
    "numerics",
)


def locate_data_repository(start: Path | None = None) -> Path:
    """从常见工作目录查找含图鉴快照的数据仓库。"""

    origin = Path.cwd() if start is None else start
    origin = origin.resolve()
    candidates: list[Path] = []
    for parent in (origin, *origin.parents):
        candidates.append(parent / "hsr-currency-wars-data")
    candidates.append(origin / "hsr-currency-wars-data")
    seen: set[Path] = set()
    for candidate in candidates:
        resolved = candidate.resolve()
        if resolved in seen:
            continue
        seen.add(resolved)
        if (resolved / "data" / "encyclopedia" / "manifest.json").is_file():
            return resolved
    raise FileNotFoundError("未找到 hsr-currency-wars-data 数据仓库")


def load_snapshot_rule_pack(
    repository: Path,
    *,
    base_pack: RulePack | None = None,
) -> RulePack:
    """校验图鉴快照并合并最新角色、羁绊和节点数据。"""

    encyclopedia = _encyclopedia_directory(repository)
    repository_root = encyclopedia.parents[1]
    manifest = _read_object(encyclopedia / "manifest.json", "manifest")
    verified = {
        name: _load_verified_json(repository_root, manifest, name) for name in SNAPSHOT_FILES
    }
    characters = _as_list(verified["characters"], "characters")
    traits = _as_list(verified["traits"], "traits")
    trait_details = _as_object(verified["trait_details"], "trait_details")
    presentation = _as_object(verified["presentation"], "presentation")
    numerics = _as_object(verified["numerics"], "numerics")

    trait_by_name, trait_ids = _parse_traits(traits, trait_details, presentation)
    units = _parse_units(characters, trait_by_name, presentation)
    nodes = _as_list(numerics.get("nodes"), "numerics.nodes")
    node_ids: list[str] = []
    node_types: list[NodeType] = []
    node_planes: list[int] = []
    for node_value in nodes:
        node = _as_object(node_value, "numerics.nodes[]")
        node_id = _as_string(node.get("id"), "numerics.nodes[].id")
        plane = _as_int(node.get("plane"), "numerics.nodes[].plane")
        kind = _as_string(node.get("kind"), "numerics.nodes[].kind")
        if plane not in (1, 2, 3):
            raise ValueError(f"节点 {node_id} 的位面非法")
        node_ids.append(node_id)
        node_planes.append(plane)
        node_types.append(NodeType(kind))
    if not node_ids or len(node_ids) != len(set(node_ids)):
        raise ValueError("节点 ID 不能为空或重复")

    base = load_rule_pack() if base_pack is None else base_pack
    metadata_data = _as_object(manifest, "manifest")
    snapshot_id = _as_string(metadata_data.get("snapshot_id"), "manifest.snapshot_id")
    content_hash = _as_string(metadata_data.get("content_sha256"), "manifest.content_sha256")
    season = _as_string(metadata_data.get("season"), "manifest.season")
    data_version = _as_string(metadata_data.get("game_version"), "manifest.game_version")
    metadata = RuleMetadata(
        public_source="hsr-currency-wars-data",
        public_revision=snapshot_id,
        retrieved_on="",
        local_client_build="4.7.0",
        validation_status=f"{season} 图鉴快照，已校验 SHA256",
        assumptions=base.metadata.assumptions + ("角色、羁绊与节点顺序来自私有数据仓库图鉴快照。",),
        snapshot_id=snapshot_id,
        content_sha256=content_hash,
    )
    rules = replace(
        base.rules,
        game_version=data_version,
        node_types=tuple(node_types),
        node_ids=tuple(node_ids),
        node_planes=tuple(node_planes),
    )
    return RulePack(
        rules=rules,
        metadata=metadata,
        units=units,
        difficulties=base.difficulties,
        affixes=base.affixes,
        investments=base.investments,
        environments=base.environments,
        traits=trait_ids,
    )


def _encyclopedia_directory(repository: Path) -> Path:
    candidate = repository.resolve()
    if candidate.is_file() and candidate.name == "manifest.json":
        return candidate.parent
    if (candidate / "manifest.json").is_file():
        return candidate
    nested = candidate / "data" / "encyclopedia"
    if (nested / "manifest.json").is_file():
        return nested
    raise FileNotFoundError(f"目录中没有 data/encyclopedia/manifest.json：{repository}")


def _load_verified_json(
    repository_root: Path,
    manifest: dict[str, object],
    name: str,
) -> object:
    files = _as_object(manifest.get("files"), "manifest.files")
    entry = _as_object(files.get(name), f"manifest.files.{name}")
    relative = _as_string(entry.get("path"), f"manifest.files.{name}.path")
    expected = _as_string(entry.get("sha256"), f"manifest.files.{name}.sha256")
    path = (repository_root / relative).resolve()
    try:
        path.relative_to(repository_root)
    except ValueError as error:
        raise ValueError(f"快照路径越界：{relative}") from error
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    if digest != expected:
        raise ValueError(f"快照文件校验失败：{name}")
    return json.loads(path.read_text(encoding="utf-8"))


def _parse_traits(
    values: list[object],
    details: dict[str, object],
    presentation: dict[str, object],
) -> tuple[dict[str, str], dict[str, TraitDefinition]]:
    images = _as_object(presentation.get("images"), "presentation.images")
    trait_images = _as_object(images.get("traits"), "presentation.images.traits")
    groups = _as_object(presentation.get("trait_groups"), "presentation.trait_groups")
    detail_map = _as_object(details.get("traits"), "trait_details.traits")
    by_name: dict[str, str] = {}
    by_id: dict[str, TraitDefinition] = {}
    for value in values:
        trait = _as_object(value, "traits[]")
        trait_id = _as_string(trait.get("id"), "traits[].id")
        name = _as_string(trait.get("name"), "traits[].name")
        description = _as_string(trait.get("description"), "traits[].description")
        if name in by_name or trait_id in by_id:
            raise ValueError("羁绊名称或 ID 重复")
        detail_value = detail_map.get(trait_id)
        detail = {} if detail_value is None else _as_object(detail_value, trait_id)
        tiers = tuple(
            _parse_trait_tier(item)
            for item in _as_list(detail.get("tiers", []), f"{trait_id}.tiers")
        )
        members = (
            tuple(
                _as_string(item, f"{trait_id}.members[]")
                for item in _as_list(detail.get("members", []), f"{trait_id}.members")
            )
            if "members" in detail
            else ()
        )
        icon_value = trait_images.get(trait_id)
        icon_path = (
            None
            if icon_value is None
            else _as_string(icon_value, f"presentation.images.traits.{trait_id}")
        )
        group_value = groups.get(trait_id, "其他")
        group = _as_string(group_value, f"presentation.trait_groups.{trait_id}")
        by_name[name] = trait_id
        by_id[trait_id] = TraitDefinition(
            trait_id=trait_id,
            name=name,
            description=description,
            group=group,
            icon_path=icon_path,
            tiers=tiers,
            members=members,
        )
    return by_name, by_id


def _parse_trait_tier(value: object) -> TraitTier:
    data = _as_object(value, "trait tier")
    return TraitTier(
        level=_as_int(data.get("level"), "trait tier.level"),
        text=_as_string(data.get("text"), "trait tier.text"),
        quality=_as_string(data.get("quality"), "trait tier.quality"),
    )


def _parse_units(
    values: list[object],
    trait_by_name: dict[str, str],
    presentation: dict[str, object],
) -> dict[str, UnitDefinition]:
    character_presentation = _as_object(presentation.get("characters"), "presentation.characters")
    units: dict[str, UnitDefinition] = {}
    for value in values:
        data = _as_object(value, "characters[]")
        unit_id = _as_string(data.get("id"), "characters[].id")
        name = _as_string(data.get("name"), "characters[].name")
        description = _as_string(data.get("description"), "characters[].description")
        costs = tuple(
            _as_int(item, f"{name}.cost[]") for item in _as_list(data.get("cost"), f"{name}.cost")
        )
        if not costs or any(cost not in (1, 2, 3, 4, 5, 7) for cost in costs):
            raise ValueError(f"{name} 的费用非法")
        roles = tuple(
            _as_string(item, f"{name}.roles[]")
            for item in _as_list(data.get("roles"), f"{name}.roles")
        )
        traits = tuple(
            _as_string(item, f"{name}.traits[]")
            for item in _as_list(data.get("traits"), f"{name}.traits")
        )
        unknown_traits = set(traits) - set(trait_by_name)
        if unknown_traits:
            raise ValueError(f"{name} 引用了未知羁绊：{sorted(unknown_traits)}")
        presentation_value = character_presentation.get(unit_id, {})
        presentation_data = _as_object(presentation_value, f"presentation.{unit_id}")
        portrait_value = presentation_data.get("portrait")
        portrait_path = (
            None
            if portrait_value is None
            else _as_string(portrait_value, f"presentation.{unit_id}.portrait")
        )
        if unit_id in units:
            raise ValueError(f"角色 ID 重复：{unit_id}")
        units[unit_id] = UnitDefinition(
            unit_id=unit_id,
            name=name,
            cost=min(costs),
            position=_position(_as_string(data.get("position"), f"{name}.position")),
            tags=roles,
            traits=traits,
            description=description,
            costs=costs,
            roles=roles,
            portrait_path=portrait_path,
            trait_ids=tuple(trait_by_name[trait] for trait in traits),
        )
    return units


def _position(value: str) -> Position:
    return {
        "前台": Position.FRONT,
        "后台": Position.BACK,
        "前后台": Position.FLEX,
    }[value]


def _read_object(path: Path, label: str) -> dict[str, object]:
    return _as_object(json.loads(path.read_text(encoding="utf-8")), label)


def _as_object(value: object, label: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise ValueError(f"{label} 必须是对象")
    return value


def _as_list(value: object, label: str) -> list[object]:
    if not isinstance(value, list):
        raise ValueError(f"{label} 必须是数组")
    return value


def _as_string(value: object, label: str) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError(f"{label} 必须是非空字符串")
    return value


def _as_int(value: object, label: str) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{label} 必须是整数")
    return value
