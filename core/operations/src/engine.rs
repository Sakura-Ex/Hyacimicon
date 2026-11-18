use super::models::{
    AffixDefinition, EnvironmentDefinition, InvestmentDefinition, NodeDefinition, NodeType,
    OperationRules, Position, RulePack, UnitInstance,
};
use super::rng::Rng;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Market {
    pub pool: HashMap<String, i32>,
    pub roster: HashMap<(String, i32), u32>,
    pub level: u8,
    pub xp: i32,
    pub offers: Vec<Option<String>>,
    pub free_rerolls: i32,
    pub locked: bool,
    pub purchase_lock_nodes: i32,
    pub free_reroll_chance: f64,
}

#[derive(Clone, Debug)]
struct PendingGold {
    node_index: usize,
    amount: i32,
}

#[derive(Clone, Debug)]
pub struct NodeResult {
    pub node_index: usize,
    pub node_type: NodeType,
    pub won: bool,
    pub progress: i32,
    pub provider: String,
    pub team_power: f64,
    pub enemy_rating: f64,
    pub health_delta: i32,
    pub gold_delta: i32,
    pub streak_after: i32,
}

#[derive(Clone, Debug)]
pub struct OperationEnvironment {
    pub pack: RulePack,
    rng: Rng,
    pub difficulty_id: String,
    pub enemy_difficulty: i32,
    pub affix_count: i32,
    pub environment: EnvironmentDefinition,
    pub health: i32,
    pub max_health: i32,
    pub gold: i32,
    pub node_index: usize,
    pub plane: u8,
    pub streak: i32,
    pub affixes: Vec<AffixDefinition>,
    pub interest_cap: i32,
    pub market: Market,
    pub front_slots: Vec<Option<UnitInstance>>,
    pub back_slots: Vec<Option<UnitInstance>>,
    pub bench_slots: Vec<Option<UnitInstance>>,
    pub selected_investments: Vec<InvestmentDefinition>,
    pub investment_offers: Vec<InvestmentDefinition>,
    pending_gold: Vec<PendingGold>,
    pub results: Vec<NodeResult>,
    pub ended: bool,
    pub won: bool,
    pub spent_this_node: bool,
    pub spent_last_node: bool,
    pub xp_cost_adjustment: i32,
    pub enemy_rating_adjustment: i32,
    pub investment_power_multiplier: f64,
    pub investment_trait_multiplier: f64,
    pub environment_power_bonus: f64,
    pub last_progress: Option<i32>,
    pub last_provider: Option<String>,
    pub battle_progress: i32,
}

#[derive(Clone, Debug)]
pub struct CreateOptions {
    pub difficulty_id: Option<String>,
    pub enemy_difficulty: Option<i32>,
    pub affix_count: Option<i32>,
    pub environment_id: Option<String>,
    pub seed: u64,
    pub battle_progress: i32,
}

impl OperationEnvironment {
    pub fn create(pack: RulePack, options: CreateOptions) -> Result<Self, String> {
        if !(0..=100).contains(&options.battle_progress) {
            return Err("战斗进度必须是 0 到 100 的整数".into());
        }
        let mut rng = Rng::new(options.seed);
        let default_difficulty = options
            .difficulty_id
            .as_deref()
            .and_then(|id| {
                pack.difficulties
                    .iter()
                    .find(|difficulty| difficulty.id == id)
            })
            .or_else(|| pack.difficulties.first())
            .ok_or_else(|| "规则包没有难度".to_string())?
            .clone();
        let enemy_difficulty = options
            .enemy_difficulty
            .unwrap_or(default_difficulty.enemy_difficulty);
        let affix_count = options
            .affix_count
            .unwrap_or(default_difficulty.affix_count);
        if !(1..=pack.rules.enemy_difficulty_max).contains(&enemy_difficulty) {
            return Err("敌人难度超出允许范围".into());
        }
        if !(0..=pack.rules.affix_count_max as i32).contains(&affix_count) {
            return Err("词缀数量超出允许范围".into());
        }
        let environment = match options.environment_id.as_deref() {
            Some(id) => pack
                .environments
                .iter()
                .find(|environment| environment.id == id)
                .cloned()
                .ok_or_else(|| format!("未知投资环境：{id}"))?,
            None => {
                let index = rng.range(pack.environments.len());
                pack.environments
                    .get(index)
                    .cloned()
                    .ok_or_else(|| "规则包没有投资环境".to_string())?
            }
        };
        let mut affix_indices: Vec<usize> = (0..pack.affixes.len()).collect();
        for index in 0..affix_count as usize {
            let swap = index + rng.range(affix_indices.len() - index);
            affix_indices.swap(index, swap);
        }
        affix_indices.truncate(affix_count as usize);
        let mut affixes: Vec<_> = affix_indices
            .into_iter()
            .filter_map(|index| pack.affixes.get(index).cloned())
            .collect();
        if environment.effect.remove_random_affix && !affixes.is_empty() {
            let index = rng.range(affixes.len());
            affixes.remove(index);
        }
        let max_health = pack.rules.starting_health;
        let health = (max_health
            + affixes
                .iter()
                .map(|affix| affix.effect.initial_health_delta)
                .sum::<i32>())
        .max(1);
        let gold = pack.rules.starting_gold + environment.effect.initial_gold;
        let difficulty_id = if options.enemy_difficulty.is_some() || options.affix_count.is_some() {
            "custom".to_owned()
        } else {
            default_difficulty.id
        };
        let mut environment_state = Self {
            market: Market::new(&pack, &mut rng),
            pack,
            rng,
            difficulty_id,
            enemy_difficulty,
            affix_count,
            environment,
            health,
            max_health,
            gold,
            node_index: 0,
            plane: 0,
            streak: 0,
            affixes,
            interest_cap: 5,
            front_slots: Vec::new(),
            back_slots: Vec::new(),
            bench_slots: Vec::new(),
            selected_investments: Vec::new(),
            investment_offers: Vec::new(),
            pending_gold: Vec::new(),
            results: Vec::new(),
            ended: false,
            won: false,
            spent_this_node: false,
            spent_last_node: false,
            xp_cost_adjustment: 0,
            enemy_rating_adjustment: 0,
            investment_power_multiplier: 1.0,
            investment_trait_multiplier: 1.0,
            environment_power_bonus: 0.0,
            last_progress: None,
            last_provider: None,
            battle_progress: options.battle_progress,
        };
        environment_state.front_slots = vec![None; environment_state.pack.rules.front_capacity];
        environment_state.back_slots = vec![None; environment_state.pack.rules.back_capacity];
        environment_state.bench_slots = vec![None; environment_state.pack.rules.bench_capacity];
        environment_state.apply_starting_units()?;
        environment_state.enter_node()?;
        Ok(environment_state)
    }

    fn apply_starting_units(&mut self) -> Result<(), String> {
        let template = if self.rng.range(2) == 0 {
            [(3, 1), (1, 3)]
        } else {
            [(2, 2), (1, 2)]
        };
        for (cost, count) in template {
            for _ in 0..count {
                let instance = self.take_random_unit_by_cost(cost)?;
                self.push_to_bench(instance)?;
            }
        }
        self.auto_deploy();
        Ok(())
    }

    fn take_random_unit_by_cost(&mut self, cost: u8) -> Result<UnitInstance, String> {
        let mut candidates: Vec<_> = self
            .market
            .pool
            .iter()
            .filter(|(id, count)| {
                **count > 0
                    && self
                        .pack
                        .units
                        .get(*id)
                        .is_some_and(|unit| unit.cost == cost)
            })
            .map(|(id, _)| id.clone())
            .collect();
        candidates.sort();
        if candidates.is_empty() {
            return Err(format!("没有可用的 {cost} 费角色"));
        }
        let unit_id = candidates[self.rng.range(candidates.len())].clone();
        self.take_unit(&unit_id)
    }

    fn take_unit(&mut self, unit_id: &str) -> Result<UnitInstance, String> {
        let pool = self
            .market
            .pool
            .get_mut(unit_id)
            .ok_or_else(|| format!("未知角色：{unit_id}"))?;
        if *pool <= 0 {
            return Err(format!("角色牌池不足：{unit_id}"));
        }
        *pool -= 1;
        self.market
            .roster
            .entry((unit_id.to_owned(), 1))
            .and_modify(|count| *count += 1)
            .or_insert(1);
        Ok(UnitInstance {
            unit_id: unit_id.to_owned(),
            star: 1,
        })
    }

    fn push_to_bench(&mut self, unit: UnitInstance) -> Result<(), String> {
        let index = first_empty(&self.bench_slots).ok_or_else(|| "备战席已满".to_string())?;
        self.bench_slots[index] = Some(unit);
        Ok(())
    }

    fn auto_deploy(&mut self) {
        for index in 0..self.bench_slots.len() {
            if self.active_count() >= self.market.active_capacity() {
                break;
            }
            let Some(unit) = self.bench_slots[index].clone() else {
                continue;
            };
            let position = self
                .pack
                .units
                .get(&unit.unit_id)
                .map(|value| value.position)
                .unwrap_or(Position::Flex);
            let candidates = match position {
                Position::Back => [Some(false), Some(true)],
                Position::Front => [Some(true), Some(false)],
                Position::Flex => [Some(true), Some(false)],
            };
            for front in candidates.into_iter().flatten() {
                let slots = if front {
                    &mut self.front_slots
                } else {
                    &mut self.back_slots
                };
                if let Some(target) = first_empty(slots) {
                    slots[target] = Some(unit.clone());
                    self.bench_slots[index] = None;
                    break;
                }
            }
        }
    }

    fn enter_node(&mut self) -> Result<(), String> {
        if self.node_index >= self.pack.nodes.len() {
            self.ended = true;
            return Ok(());
        }
        let node = self
            .current_node()
            .cloned()
            .ok_or_else(|| "节点不存在".to_string())?;
        let previous_plane = self.plane;
        self.plane = node.plane;
        if self.plane != previous_plane {
            self.gold += self
                .environment
                .effect
                .plane_gold
                .get(self.plane as usize - 1)
                .copied()
                .unwrap_or(0);
            if self.plane > 1 {
                for investment in self.selected_investments.clone() {
                    self.market.free_rerolls += investment.effect.plane_free_rerolls;
                    self.grant_xp(investment.effect.plane_xp);
                }
            }
        }
        for investment in self.selected_investments.clone() {
            self.market.free_rerolls += investment.effect.node_free_rerolls;
            if node.node_type == NodeType::Boss {
                self.gold += investment.effect.boss_gold;
            }
            if self.node_index == 16 {
                self.gold += investment.effect.last_supply_gold;
            }
        }
        if !self.spent_last_node {
            for investment in self.selected_investments.clone() {
                self.market.free_rerolls += investment.effect.idle_node_free_rerolls;
                if investment.effect.idle_node_gold_from_interest {
                    self.gold +=
                        (self.gold / self.pack.rules.interest_step_gold).min(self.interest_cap);
                }
            }
        }
        let due: Vec<_> = self
            .pending_gold
            .iter()
            .filter(|reward| reward.node_index == self.node_index)
            .cloned()
            .collect();
        for reward in &due {
            self.gold += reward.amount;
        }
        self.pending_gold
            .retain(|reward| reward.node_index != self.node_index);
        self.environment_power_bonus += self.environment.effect.team_power_per_node;
        if !self.market.locked {
            self.market.roll_shop(&self.pack, &mut self.rng);
        }
        if self.pack.rules.investment_nodes.contains(&self.node_index) {
            self.open_investment_offers();
        }
        Ok(())
    }

    pub fn current_node(&self) -> Option<&NodeDefinition> {
        self.pack.nodes.get(self.node_index)
    }

    pub fn active_count(&self) -> usize {
        self.front_slots.iter().flatten().count() + self.back_slots.iter().flatten().count()
    }

    pub fn bench_count(&self) -> usize {
        self.bench_slots.iter().flatten().count()
    }

    pub fn active_units(&self) -> Vec<UnitInstance> {
        self.front_slots
            .iter()
            .flatten()
            .chain(self.back_slots.iter().flatten())
            .cloned()
            .collect()
    }

    pub fn buy_shop_offer(&mut self, index: usize) -> Result<Vec<String>, String> {
        self.ensure_market()?;
        let unit_id = self
            .market
            .offers
            .get(index)
            .and_then(Clone::clone)
            .ok_or_else(|| "商店位置没有角色".to_string())?;
        let unit = self
            .pack
            .units
            .get(&unit_id)
            .ok_or_else(|| "未知角色".to_string())?;
        if self.gold < unit.cost as i32 {
            return Err("金币不足，无法购买该角色".into());
        }
        if self.bench_count() >= self.pack.rules.bench_capacity
            && !self.purchase_would_merge(&unit_id)
        {
            return Err("备战席已满".into());
        }
        self.gold -= unit.cost as i32;
        if let Some(pool) = self.market.pool.get_mut(&unit_id) {
            *pool -= 1;
        }
        let merges = add_to_roster(&mut self.market.roster, &unit_id);
        self.market.offers[index] = None;
        if merges.is_empty() {
            self.push_to_bench(UnitInstance {
                unit_id: unit_id.clone(),
                star: 1,
            })?;
        } else {
            self.reconcile_slots();
        }
        self.spent_this_node = true;
        Ok(merges)
    }

    pub fn sell_unit(&mut self, unit_id: &str, star: i32) -> Result<i32, String> {
        self.ensure_market()?;
        let unit_cost = self
            .pack
            .units
            .get(unit_id)
            .map(|unit| unit.cost as i32)
            .ok_or_else(|| "未知角色".to_string())?;
        let key = (unit_id.to_owned(), star);
        let count = self.market.roster.get(&key).copied().unwrap_or(0);
        if count == 0 {
            return Err("没有可出售的指定角色".into());
        }
        if count == 1 {
            self.market.roster.remove(&key);
        } else {
            self.market.roster.insert(key, count - 1);
        }
        let returned = 3_i32.pow((star - 1) as u32);
        if let Some(pool) = self.market.pool.get_mut(unit_id) {
            *pool += returned;
        }
        self.gold += unit_cost * returned;
        self.reconcile_slots();
        Ok(unit_cost * returned)
    }

    pub fn reroll_shop(&mut self) -> Result<bool, String> {
        self.ensure_market()?;
        if self.market.purchase_lock_nodes > 0 {
            return Err("当前投资策略禁用了商店购买和刷新".into());
        }
        let paid = if self.market.free_rerolls > 0 {
            self.market.free_rerolls -= 1;
            false
        } else {
            if self.gold < self.pack.rules.refresh_cost {
                return Err("金币不足，无法刷新商店".into());
            }
            self.gold -= self.pack.rules.refresh_cost;
            true
        };
        self.market.roll_shop(&self.pack, &mut self.rng);
        if paid
            && self.market.free_reroll_chance > 0.0
            && self.rng.random() < self.market.free_reroll_chance
        {
            self.market.free_rerolls += 1;
        }
        self.spent_this_node |= paid;
        Ok(paid)
    }

    pub fn buy_xp(&mut self, count: i32) -> Result<Vec<u8>, String> {
        self.ensure_market()?;
        if count < 1 {
            return Err("购买次数必须大于 0".into());
        }
        if self.market.level >= 10 {
            return Err("角色等级已达到上限".into());
        }
        let extra = self
            .affixes
            .iter()
            .filter(|affix| {
                affix.effect.xp_extra_cost_after_level > 0
                    && self.market.level as i32 >= affix.effect.xp_extra_cost_after_level
            })
            .map(|affix| affix.effect.xp_extra_cost)
            .sum::<i32>();
        let cost = (self.pack.rules.xp_purchase_cost + self.xp_cost_adjustment + extra).max(0);
        if self.gold < cost * count {
            return Err("金币不足，无法购买经验".into());
        }
        let mut levels = Vec::new();
        for _ in 0..count {
            if self.market.level >= 10 {
                break;
            }
            self.gold -= cost;
            self.market.xp += self.pack.rules.xp_purchase_amount;
            levels.extend(self.advance_levels());
        }
        self.spent_this_node = true;
        Ok(levels)
    }

    pub fn set_shop_locked(&mut self, locked: bool) {
        self.market.locked = locked;
    }

    pub fn choose_investment(&mut self, index: usize) -> Result<(), String> {
        if self.investment_offers.is_empty() {
            return Err("当前没有投资策略".into());
        }
        let investment = self
            .investment_offers
            .get(index)
            .cloned()
            .ok_or_else(|| "投资策略位置不存在".to_string())?;
        self.apply_investment(&investment);
        self.investment_offers.clear();
        self.selected_investments.push(investment);
        Ok(())
    }

    pub fn set_unit_position(
        &mut self,
        unit_id: &str,
        star: i32,
        position: Option<Position>,
    ) -> Result<(), String> {
        self.ensure_market()?;
        let unit = UnitInstance {
            unit_id: unit_id.to_owned(),
            star,
        };
        let source = self.find_slot(&unit);
        if source.is_none() {
            return Err("没有可部署的指定角色".into());
        }
        let (source_location, source_index) = source.expect("已检查存在");
        match position {
            None => {
                if source_location == "bench" {
                    return Ok(());
                }
                let target =
                    first_empty(&self.bench_slots).ok_or_else(|| "备战席已满".to_string())?;
                self.bench_slots[target] = Some(unit);
                self.slots_mut(source_location)[source_index] = None;
            }
            Some(Position::Flex) => {
                return Err("角色只能部署到前台或后台".into());
            }
            Some(target_position) => {
                if !can_place(&self.pack, &unit, target_position) {
                    return Err("角色站位不符合前台或后台要求".into());
                }
                let target_location = if target_position == Position::Front {
                    "front"
                } else {
                    "back"
                };
                let target_index = first_empty(self.slots_mut(target_location))
                    .ok_or_else(|| "目标区域已满".to_string())?;
                if source_location == "bench"
                    && self.active_count() >= self.market.active_capacity()
                {
                    return Err("上阵人数已达到商店等级上限".into());
                }
                self.slots_mut(source_location)[source_index] = None;
                self.slots_mut(target_location)[target_index] = Some(unit);
            }
        }
        Ok(())
    }

    pub fn swap_unit_slots(
        &mut self,
        source_location: &str,
        source_index: usize,
        target_location: &str,
        target_index: usize,
    ) -> Result<(), String> {
        let source_slots = match source_location {
            "front" => &self.front_slots,
            "back" => &self.back_slots,
            "bench" => &self.bench_slots,
            _ => return Err("未知拖动起点".into()),
        };
        let target_slots = match target_location {
            "front" => &self.front_slots,
            "back" => &self.back_slots,
            "bench" => &self.bench_slots,
            _ => return Err("未知拖动终点".into()),
        };
        let source = source_slots
            .get(source_index)
            .and_then(Clone::clone)
            .ok_or_else(|| "拖动起点没有角色".to_string())?;
        let target = target_slots.get(target_index).and_then(Clone::clone);
        if let Some(position) = location_position(target_location)
            && !can_place(&self.pack, &source, position)
        {
            return Err("角色站位不符合目标区域要求".into());
        }
        if let Some(target_unit) = &target
            && let Some(position) = location_position(source_location)
            && !can_place(&self.pack, target_unit, position)
        {
            return Err("交换后角色站位不合法".into());
        }
        let next_active = self.active_count()
            + usize::from(
                source_location == "bench" && target_location != "bench" && target.is_none(),
            )
            - usize::from(
                source_location != "bench" && target_location == "bench" && target.is_none(),
            );
        if next_active > self.market.active_capacity() {
            return Err("上阵人数已达到商店等级上限".into());
        }
        let (front, back, bench) = (
            self.front_slots.clone(),
            self.back_slots.clone(),
            self.bench_slots.clone(),
        );
        self.slots_mut(source_location)[source_index] = target;
        self.slots_mut(target_location)[target_index] = Some(source);
        if !self.slots_match_roster() {
            self.front_slots = front;
            self.back_slots = back;
            self.bench_slots = bench;
            return Err("交换后角色归属不合法".into());
        }
        Ok(())
    }

    pub fn advance_node(&mut self) -> Result<NodeResult, String> {
        if self.ended {
            return Err("对局已经结束".into());
        }
        if !self.investment_offers.is_empty() {
            return Err("需要先选择一个投资策略".into());
        }
        let node = self
            .current_node()
            .cloned()
            .ok_or_else(|| "当前节点不存在".to_string())?;
        let team_power = self.team_power();
        let enemy_rating = self.enemy_rating(&node);
        let won = self.battle_progress == 100;
        let old_health = self.health;
        let old_gold = self.gold;
        if won {
            self.streak += 1;
        } else {
            self.streak = 0;
        }
        let interest = (self.gold / self.pack.rules.interest_step_gold).min(self.interest_cap);
        let streak_bonus = if won { (1 + self.streak / 2).min(5) } else { 0 };
        let mut base_gold = self.pack.rules.base_battle_gold;
        if node.node_type == NodeType::Reward {
            base_gold =
                (base_gold as f64 * self.environment.effect.reward_multiplier).round() as i32;
        }
        self.gold += base_gold + interest + streak_bonus;
        if !won {
            self.health =
                (self.health - self.pack.rules.loss_damage[node.node_type.loss_index()]).max(0);
        }
        let result = NodeResult {
            node_index: self.node_index,
            node_type: node.node_type,
            won,
            progress: self.battle_progress,
            provider: "fixed".into(),
            team_power,
            enemy_rating,
            health_delta: self.health - old_health,
            gold_delta: self.gold - old_gold,
            streak_after: self.streak,
        };
        self.results.push(result.clone());
        self.last_progress = Some(self.battle_progress);
        self.last_provider = Some("fixed".into());
        if self.health <= 0 || self.node_index + 1 == self.pack.nodes.len() {
            self.ended = true;
            self.won = won && self.health > 0;
            return Ok(result);
        }
        self.spent_last_node = self.spent_this_node;
        self.spent_this_node = false;
        self.node_index += 1;
        self.enter_node()?;
        Ok(result)
    }

    pub fn set_battle_progress(&mut self, progress: i32) -> Result<(), String> {
        if !(0..=100).contains(&progress) {
            return Err("战斗进度必须是 0 到 100 的整数".into());
        }
        self.battle_progress = progress;
        Ok(())
    }

    pub fn state_json(&self, session_id: &str, seed: u64, events: &[Value]) -> Value {
        let node = self.current_node();
        let node_number = (self.node_index + 1).min(self.pack.nodes.len());
        let current_node_type = node.map(|node| node.node_type).unwrap_or(NodeType::Boss);
        let active_units = self.active_units();
        let front_slots = slot_views(&self.pack, &self.front_slots);
        let back_slots = slot_views(&self.pack, &self.back_slots);
        let bench_slots = slot_views(&self.pack, &self.bench_slots);
        let selected_investments: Vec<_> = self
            .selected_investments
            .iter()
            .map(investment_view)
            .collect();
        let offers: Vec<_> = self.investment_offers.iter().map(investment_view).collect();
        let mut pool: Vec<_> = self
            .market
            .pool
            .iter()
            .filter_map(|(id, remaining)| {
                let unit = self.pack.units.get(id)?;
                Some((unit.cost, id.clone(), unit.name.clone(), *remaining))
            })
            .collect();
        pool.sort();
        let phase = if self.ended {
            "finished"
        } else if !self.investment_offers.is_empty() {
            "investment"
        } else {
            "operations"
        };
        json!({
            "session_id": session_id,
            "seed": seed,
            "game_version": self.pack.game_version,
            "snapshot_id": self.pack.snapshot_id,
            "phase": phase,
            "finished": self.ended,
            "won": self.won,
            "node": {
                "number": node_number,
                "total": self.pack.nodes.len(),
                "index": self.node_index,
                "id": node.map(|value| value.id.clone()).unwrap_or_default(),
                "type": current_node_type,
                "plane": self.plane,
            },
            "node_template": self.pack.nodes.iter().enumerate().map(|(index, node)| json!({
                "index": index,
                "id": node.id,
                "type": node.node_type,
                "plane": node.plane,
            })).collect::<Vec<_>>(),
            "resources": {
                "health": self.health,
                "max_health": self.max_health,
                "gold": self.gold,
                "level": self.market.level,
                "xp": self.market.xp,
                "xp_to_next_level": self.market.xp_to_next(&self.pack.rules),
                "streak": self.streak,
                "free_rerolls": self.market.free_rerolls,
            },
            "shop": self.market.offers.iter().map(|unit_id| match unit_id {
                Some(unit_id) => {
                    let unit = self.pack.units.get(unit_id).expect("商店角色存在");
                    json!({
                        "empty": false,
                        "unit_id": unit.id,
                        "name": unit.name,
                        "cost": unit.cost,
                        "position": unit.position,
                        "traits": unit.traits,
                        "trait_ids": unit.trait_ids,
                        "costs": unit.costs,
                        "roles": unit.roles,
                        "portrait_path": unit.portrait_path,
                        "description": unit.description,
                        "affordable": unit.cost as i32 <= self.gold,
                    })
                }
                None => json!({"empty": true}),
            }).collect::<Vec<_>>(),
            "shop_locked": self.market.locked,
            "roster": self.market.roster.iter().flat_map(|((id, star), count)| {
                (0..*count).filter_map(|_| self.pack.units.get(id).map(|_| unit_view(&self.pack, &UnitInstance { unit_id: id.clone(), star: *star }))).collect::<Vec<_>>()
            }).collect::<Vec<_>>(),
            "active_units": active_units.iter().map(|unit| unit_view(&self.pack, unit)).collect::<Vec<_>>(),
            "front_units": self.front_slots.iter().flatten().map(|unit| unit_view(&self.pack, unit)).collect::<Vec<_>>(),
            "back_units": self.back_slots.iter().flatten().map(|unit| unit_view(&self.pack, unit)).collect::<Vec<_>>(),
            "bench_units": self.bench_slots.iter().flatten().map(|unit| unit_view(&self.pack, unit)).collect::<Vec<_>>(),
            "front_slots": front_slots,
            "back_slots": back_slots,
            "bench_slots": bench_slots,
            "team_limits": {
                "active": self.market.active_capacity(),
                "front": self.pack.rules.front_capacity,
                "back": self.pack.rules.back_capacity,
                "bench": self.pack.rules.bench_capacity,
                "bench_used": self.bench_count(),
            },
            "active_traits": self.active_trait_view(),
            "investments": {
                "selected": selected_investments,
                "offers": offers,
            },
            "environment": {
                "id": self.environment.id,
                "name": self.environment.name,
                "description": self.environment.description,
            },
            "difficulty": {
                "id": self.difficulty_id,
                "name": if self.difficulty_id == "custom" {
                    "自定义".to_owned()
                } else {
                    self.pack
                        .difficulties
                        .iter()
                        .find(|difficulty| difficulty.id == self.difficulty_id)
                        .map(|difficulty| difficulty.name.clone())
                        .unwrap_or_else(|| self.difficulty_id.clone())
                },
                "enemy_difficulty": self.enemy_difficulty,
                "affix_count": self.affix_count,
            },
            "affixes": self.affixes.iter().map(|affix| json!({
                "id": affix.id,
                "name": affix.name,
                "description": affix.description,
            })).collect::<Vec<_>>(),
            "battle": {
                "provider_id": "fixed",
                "progress_percent": self.battle_progress,
                "last_progress": self.last_progress,
                "last_provider": self.last_provider,
            },
            "capabilities": {
                "set_battle_progress": true,
                "lock_shop": !self.ended && self.investment_offers.is_empty(),
                "reroll_shop": !self.ended && self.investment_offers.is_empty() && (self.market.free_rerolls > 0 || self.gold >= self.pack.rules.refresh_cost),
                "buy_offer": !self.ended && self.investment_offers.is_empty() && self.market.offers.iter().any(Option::is_some),
                "sell_unit": !self.ended && self.investment_offers.is_empty() && !self.market.roster.is_empty(),
                "set_unit_position": !self.ended && self.investment_offers.is_empty() && !self.market.roster.is_empty(),
                "buy_xp": !self.ended && self.investment_offers.is_empty() && self.market.level < 10,
                "choose_investment": !self.investment_offers.is_empty(),
                "advance_node": !self.ended && self.investment_offers.is_empty(),
            },
            "results": self.results.iter().map(|result| json!({
                "node_index": result.node_index,
                "node_type": result.node_type,
                "won": result.won,
                "battle_progress": result.progress,
                "battle_provider": result.provider,
                "team_power": (result.team_power * 1000.0).round() / 1000.0,
                "enemy_rating": (result.enemy_rating * 1000.0).round() / 1000.0,
                "health_delta": result.health_delta,
                "gold_delta": result.gold_delta,
                "streak_after": result.streak_after,
            })).collect::<Vec<_>>(),
            "events": events,
            "debug": {
                "team_power": self.team_power(),
                "node_enemy_rating": if self.ended { None } else { node.map(|node| self.enemy_rating(node)) },
                "pool": pool.into_iter().map(|(cost, unit_id, name, remaining)| json!({
                    "unit_id": unit_id,
                    "name": name,
                    "cost": cost,
                    "remaining": remaining,
                })).collect::<Vec<_>>(),
            },
        })
    }

    fn open_investment_offers(&mut self) {
        let mut candidates: Vec<_> = self
            .pack
            .investments
            .iter()
            .filter(|investment| investment.planes.contains(&self.plane))
            .cloned()
            .collect();
        if candidates.is_empty() {
            self.investment_offers.clear();
            return;
        }
        for index in 0..candidates.len().min(3) {
            let swap = index + self.rng.range(candidates.len() - index);
            candidates.swap(index, swap);
        }
        candidates.truncate(3);
        if let Some(first) = self.environment.effect.first_strategy_rarity {
            candidates[0].rarity = first;
        } else if let Some(mode) = self.environment.effect.strategy_rarity_mode {
            for investment in &mut candidates {
                investment.rarity = mode;
            }
        }
        self.investment_offers = candidates;
    }

    fn apply_investment(&mut self, investment: &InvestmentDefinition) {
        let effect = &investment.effect;
        self.gold += effect.gold;
        self.market.free_rerolls += effect.free_rerolls;
        if let Some(interest_cap) = effect.interest_cap {
            self.interest_cap = self.interest_cap.max(interest_cap);
        }
        if let Some(streak) = effect.set_streak {
            self.streak = streak;
        }
        self.market.purchase_lock_nodes = self
            .market
            .purchase_lock_nodes
            .max(effect.lock_purchase_nodes);
        if effect.deferred_gold > 0 {
            self.pending_gold.push(PendingGold {
                node_index: self.node_index + effect.lock_purchase_nodes as usize,
                amount: effect.deferred_gold,
            });
        }
        if effect.node_gold > 0 {
            self.gold += effect.node_gold;
            for offset in 1..=effect.node_gold_steps as usize {
                self.pending_gold.push(PendingGold {
                    node_index: self.node_index + offset,
                    amount: effect.node_gold,
                });
            }
        }
        self.market.free_reroll_chance += effect.free_reroll_chance;
        self.xp_cost_adjustment += effect.xp_cost_delta;
        self.investment_power_multiplier *= effect.team_power_multiplier;
        self.investment_trait_multiplier *= effect.trait_power_multiplier;
        self.enemy_rating_adjustment +=
            investment.rarity.enemy_rating_delta() + effect.enemy_rating_delta;
        self.grant_xp(effect.xp);
        if effect.swap_health_gold {
            std::mem::swap(&mut self.health, &mut self.gold);
        }
    }

    fn grant_xp(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.market.xp += amount;
        self.advance_levels();
    }

    fn advance_levels(&mut self) -> Vec<u8> {
        let mut levels = Vec::new();
        while self.market.level < 10 {
            let required =
                self.pack.rules.xp_to_next_level[self.market.level.saturating_sub(1) as usize];
            if self.market.xp < required {
                break;
            }
            self.market.xp -= required;
            self.market.level += 1;
            levels.push(self.market.level);
        }
        levels
    }

    fn ensure_market(&self) -> Result<(), String> {
        if self.ended {
            return Err("对局已经结束".into());
        }
        if !self.investment_offers.is_empty() {
            return Err("需要先选择一个投资策略".into());
        }
        Ok(())
    }

    fn team_power(&self) -> f64 {
        let active = self.active_units();
        let mut trait_counts: HashMap<&str, i32> = HashMap::new();
        let mut unit_power = 0.0;
        for unit in &active {
            if let Some(definition) = self.pack.units.get(&unit.unit_id) {
                let affix_cost = self.affixes.iter().fold(1.0, |value, affix| {
                    value
                        * affix
                            .effect
                            .cost_power_multipliers
                            .get(definition.cost as usize - 1)
                            .copied()
                            .unwrap_or(1.0)
                });
                let affix_star = self.affixes.iter().fold(1.0, |value, affix| {
                    value
                        * affix
                            .effect
                            .star_power_multipliers
                            .get((unit.star - 1).clamp(0, 2) as usize)
                            .copied()
                            .unwrap_or(1.0)
                });
                unit_power += definition.cost as f64
                    * (1.0 + (unit.star - 1) as f64 * 1.5)
                    * affix_cost
                    * affix_star;
                for trait_name in &definition.traits {
                    *trait_counts.entry(trait_name).or_default() += 1;
                }
            }
        }
        let trait_power: f64 = trait_counts
            .values()
            .filter(|count| **count >= 2)
            .map(|count| (((count - 1) as f64) * 0.45).min(2.0))
            .sum();
        let affix_team = self
            .affixes
            .iter()
            .map(|affix| affix.effect.team_power_multiplier)
            .product::<f64>();
        (unit_power * affix_team
            + trait_power * self.investment_trait_multiplier * affix_team
            + self.environment_power_bonus)
            * self.investment_power_multiplier
    }

    fn enemy_rating(&self, node: &NodeDefinition) -> f64 {
        let affix_multiplier = self
            .affixes
            .iter()
            .map(|affix| {
                affix.effect.enemy_rating_multiplier
                    * affix
                        .effect
                        .plane_enemy_rating_multipliers
                        .get(node.plane as usize - 1)
                        .copied()
                        .unwrap_or(1.0)
            })
            .product::<f64>();
        (self.enemy_difficulty as f64 * node.node_type.strength_multiplier() * affix_multiplier
            + self.enemy_rating_adjustment as f64)
            .max(0.1)
    }

    fn active_trait_view(&self) -> Vec<Value> {
        let mut counts: HashMap<&str, i32> = HashMap::new();
        for unit in self.active_units() {
            if let Some(definition) = self.pack.units.get(&unit.unit_id) {
                for trait_id in &definition.trait_ids {
                    *counts.entry(trait_id).or_default() += 1;
                }
            }
        }
        let mut values: Vec<_> = counts
            .into_iter()
            .filter_map(|(trait_id, count)| {
                let definition = self.pack.traits.get(trait_id)?;
                let active_tier = definition
                    .tiers
                    .iter()
                    .filter(|tier| tier.level <= count)
                    .max_by_key(|tier| tier.level);
                let next_tier = definition
                    .tiers
                    .iter()
                    .filter(|tier| tier.level > count)
                    .min_by_key(|tier| tier.level);
                Some(json!({
                    "id": definition.id,
                    "name": definition.name,
                    "group": definition.group,
                    "icon_path": definition.icon_path,
                    "count": count,
                    "active_tier": active_tier.map(|tier| tier.level),
                    "next_tier": next_tier.map(|tier| tier.level),
                    "active_text": active_tier.map(|tier| tier.text.clone()).unwrap_or_default(),
                    "next_text": next_tier.map(|tier| tier.text.clone()).unwrap_or_default(),
                    "status": if active_tier.is_some() { "active" } else { "inactive" },
                }))
            })
            .collect();
        values.sort_by(|left, right| {
            let left_count = left["count"].as_i64().unwrap_or(0);
            let right_count = right["count"].as_i64().unwrap_or(0);
            right_count
                .cmp(&left_count)
                .then_with(|| left["name"].as_str().cmp(&right["name"].as_str()))
        });
        values
    }

    fn purchase_would_merge(&self, unit_id: &str) -> bool {
        (1..4).any(|star| {
            self.market
                .roster
                .get(&(unit_id.to_owned(), star))
                .copied()
                .unwrap_or(0)
                >= 2
        })
    }

    fn reconcile_slots(&mut self) {
        for key in all_keys(&self.front_slots, &self.back_slots, &self.bench_slots) {
            let owned = self.market.roster.get(&key).copied().unwrap_or(0) as usize;
            while slot_count(&self.front_slots, &self.back_slots, &self.bench_slots, &key) > owned {
                if !remove_last_occurrence(&mut self.bench_slots, &key)
                    && !remove_last_occurrence(&mut self.back_slots, &key)
                    && !remove_last_occurrence(&mut self.front_slots, &key)
                {
                    break;
                }
            }
        }
        let placed = slot_counts(&self.front_slots, &self.back_slots, &self.bench_slots);
        for ((id, star), count) in self.market.roster.clone() {
            let missing =
                count as usize - placed.get(&(id.clone(), star)).copied().unwrap_or(0) as usize;
            for _ in 0..missing {
                if let Some(index) = first_empty(&self.bench_slots) {
                    self.bench_slots[index] = Some(UnitInstance {
                        unit_id: id.clone(),
                        star,
                    });
                }
            }
        }
    }

    fn find_slot(&self, unit: &UnitInstance) -> Option<(&'static str, usize)> {
        for (location, slots) in [
            ("front", &self.front_slots),
            ("back", &self.back_slots),
            ("bench", &self.bench_slots),
        ] {
            if let Some(index) = slots.iter().position(|value| value.as_ref() == Some(unit)) {
                return Some((location, index));
            }
        }
        None
    }

    fn slots_mut(&mut self, location: &str) -> &mut Vec<Option<UnitInstance>> {
        match location {
            "front" => &mut self.front_slots,
            "back" => &mut self.back_slots,
            "bench" => &mut self.bench_slots,
            _ => &mut self.bench_slots,
        }
    }

    fn slots_match_roster(&self) -> bool {
        slot_counts(&self.front_slots, &self.back_slots, &self.bench_slots) == self.market.roster
    }
}

impl Market {
    fn new(pack: &RulePack, rng: &mut Rng) -> Self {
        let mut market = Self {
            pool: pack
                .units
                .iter()
                .map(|(id, unit)| {
                    (
                        id.clone(),
                        pack.rules.card_copies_by_cost[unit.cost as usize - 1],
                    )
                })
                .collect(),
            roster: HashMap::new(),
            level: pack.rules.starting_level,
            xp: 0,
            offers: Vec::new(),
            free_rerolls: 0,
            locked: false,
            purchase_lock_nodes: 0,
            free_reroll_chance: 0.0,
        };
        market.roll_shop(pack, rng);
        market
    }

    fn active_capacity(&self) -> usize {
        self.level as usize
    }

    fn xp_to_next(&self, rules: &OperationRules) -> Option<i32> {
        if self.level >= 10 {
            None
        } else {
            rules.xp_to_next_level.get(self.level as usize - 1).copied()
        }
    }

    fn roll_shop(&mut self, pack: &RulePack, rng: &mut Rng) {
        let odds =
            pack.rules.shop_odds[(self.level as usize - 1).min(pack.rules.shop_odds.len() - 1)];
        let mut offers = Vec::with_capacity(pack.rules.shop_size);
        for _ in 0..pack.rules.shop_size {
            let cost_index = rng.weighted(&odds);
            let cost = cost_index + 1;
            let mut candidates: Vec<_> = pack
                .units
                .iter()
                .filter(|(id, unit)| {
                    unit.cost as usize == cost && self.pool.get(*id).copied().unwrap_or(0) > 0
                })
                .collect();
            candidates.sort_by(|left, right| left.0.cmp(right.0));
            if candidates.is_empty() {
                offers.push(None);
                continue;
            }
            let weights: Vec<f64> = candidates
                .iter()
                .map(|(id, _)| self.pool.get(*id).copied().unwrap_or(0) as f64)
                .collect();
            let selected = rng.weighted(&weights);
            offers.push(candidates.get(selected).map(|(id, _)| (*id).clone()));
        }
        self.offers = offers;
    }
}

fn add_to_roster(roster: &mut HashMap<(String, i32), u32>, unit_id: &str) -> Vec<String> {
    let mut merges = Vec::new();
    let mut star = 1;
    let mut key = (unit_id.to_owned(), star);
    let count = roster.entry(key.clone()).or_insert(0);
    *count += 1;
    while roster.get(&key).copied().unwrap_or(0) >= 3 && star < 4 {
        let count = roster.get_mut(&key).expect("合成角色存在");
        *count -= 3;
        if *count == 0 {
            roster.remove(&key);
        }
        star += 1;
        key = (unit_id.to_owned(), star);
        *roster.entry(key.clone()).or_insert(0) += 1;
        merges.push(format!("{unit_id}:{star}"));
    }
    merges
}

fn can_place(pack: &RulePack, unit: &UnitInstance, position: Position) -> bool {
    let Some(definition) = pack.units.get(&unit.unit_id) else {
        return false;
    };
    match position {
        Position::Front => definition.position != Position::Back,
        Position::Back => definition.position != Position::Front,
        Position::Flex => true,
    }
}

fn location_position(location: &str) -> Option<Position> {
    match location {
        "front" => Some(Position::Front),
        "back" => Some(Position::Back),
        _ => None,
    }
}

fn first_empty(slots: &[Option<UnitInstance>]) -> Option<usize> {
    slots.iter().position(Option::is_none)
}

fn slot_views(pack: &RulePack, slots: &[Option<UnitInstance>]) -> Vec<Value> {
    slots
        .iter()
        .map(|unit| match unit {
            Some(unit) => unit_view(pack, unit),
            None => Value::Null,
        })
        .collect()
}

fn unit_view(pack: &RulePack, unit: &UnitInstance) -> Value {
    let definition = pack.units.get(&unit.unit_id).expect("角色存在");
    json!({
        "unit_id": definition.id,
        "name": definition.name,
        "star": unit.star,
        "cost": definition.cost,
        "position": definition.position,
        "traits": definition.traits,
        "trait_ids": definition.trait_ids,
        "costs": definition.costs,
        "roles": definition.roles,
        "portrait_path": definition.portrait_path,
        "description": definition.description,
    })
}

fn investment_view(investment: &InvestmentDefinition) -> Value {
    json!({
        "id": investment.id,
        "name": investment.name,
        "rarity": investment.rarity,
        "description": investment.description,
    })
}

fn all_keys(
    front: &[Option<UnitInstance>],
    back: &[Option<UnitInstance>],
    bench: &[Option<UnitInstance>],
) -> HashSet<(String, i32)> {
    front
        .iter()
        .chain(back)
        .chain(bench)
        .flatten()
        .map(|unit| (unit.unit_id.clone(), unit.star))
        .collect()
}

fn slot_counts(
    front: &[Option<UnitInstance>],
    back: &[Option<UnitInstance>],
    bench: &[Option<UnitInstance>],
) -> HashMap<(String, i32), u32> {
    let mut counts = HashMap::new();
    for unit in front.iter().chain(back).chain(bench).flatten() {
        *counts.entry((unit.unit_id.clone(), unit.star)).or_insert(0) += 1;
    }
    counts
}

fn slot_count(
    front: &[Option<UnitInstance>],
    back: &[Option<UnitInstance>],
    bench: &[Option<UnitInstance>],
    key: &(String, i32),
) -> usize {
    front
        .iter()
        .chain(back)
        .chain(bench)
        .flatten()
        .filter(|unit| &(unit.unit_id.clone(), unit.star) == key)
        .count()
}

fn remove_last_occurrence(slots: &mut [Option<UnitInstance>], key: &(String, i32)) -> bool {
    for index in (0..slots.len()).rev() {
        if slots[index]
            .as_ref()
            .is_some_and(|unit| &(unit.unit_id.clone(), unit.star) == key)
        {
            slots[index] = None;
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::super::data::{discover_data_root, load_rule_pack};
    use super::*;

    fn environment(seed: u64) -> OperationEnvironment {
        let pack = load_rule_pack(&discover_data_root().unwrap()).unwrap();
        OperationEnvironment::create(
            pack,
            CreateOptions {
                difficulty_id: Some("standard".into()),
                enemy_difficulty: Some(20),
                affix_count: Some(2),
                environment_id: Some("silver_age".into()),
                seed,
                battle_progress: 100,
            },
        )
        .unwrap()
    }

    #[test]
    fn starting_templates_and_gold_match_rules() {
        for seed in 0..20 {
            let environment = environment(seed);
            let all_units: Vec<_> = environment
                .front_slots
                .iter()
                .chain(&environment.back_slots)
                .chain(&environment.bench_slots)
                .flatten()
                .collect();
            assert_eq!(all_units.len(), 4);
            let costs: Vec<_> = all_units
                .iter()
                .map(|unit| environment.pack.units[&unit.unit_id].cost)
                .collect();
            let template_a =
                costs.iter().filter(|cost| **cost == 1).count() == 3 && costs.contains(&3);
            let template_b = costs.iter().filter(|cost| **cost == 1).count() == 2
                && costs.iter().filter(|cost| **cost == 2).count() == 2;
            assert!(template_a || template_b, "unexpected template: {costs:?}");
            assert_eq!(environment.gold, 3);
        }
    }

    #[test]
    fn latest_snapshot_and_custom_difficulty_are_loaded() {
        let data_root = discover_data_root().unwrap();
        let pack = load_rule_pack(&data_root).unwrap();
        assert_eq!(pack.snapshot_id, "encyclopedia-4.6.54-85a54e5115f8");
        assert_eq!(pack.units.len(), 61);
        assert_eq!(pack.traits.len(), 38);
        assert_eq!(pack.affixes.len(), 53);
        assert_eq!(pack.nodes.first().unwrap().id, "1-1");
        assert_eq!(pack.nodes.first().unwrap().node_type, NodeType::Reward);
        assert_eq!(pack.nodes.last().unwrap().id, "3-7");

        let environment = OperationEnvironment::create(
            pack,
            CreateOptions {
                difficulty_id: None,
                enemy_difficulty: Some(120),
                affix_count: Some(53),
                environment_id: Some("first_prize".into()),
                seed: 7,
                battle_progress: 100,
            },
        )
        .unwrap();
        assert_eq!(environment.enemy_difficulty, 120);
        assert_eq!(environment.affixes.len(), 53);
        assert_eq!(environment.gold, 3);
        let starting_units = environment
            .front_slots
            .iter()
            .chain(&environment.back_slots)
            .chain(&environment.bench_slots)
            .flatten()
            .count();
        assert_eq!(starting_units, 4);
    }

    #[test]
    fn occupied_slots_swap_and_invalid_position_is_rejected() {
        let (mut environment, front_index, front, bench_index, bench) = (0..100)
            .find_map(|seed| {
                let environment = environment(seed);
                let (front_index, front) = environment
                    .front_slots
                    .iter()
                    .enumerate()
                    .find_map(|(index, unit)| unit.clone().map(|unit| (index, unit)))?;
                let (bench_index, bench) =
                    environment
                        .bench_slots
                        .iter()
                        .enumerate()
                        .find_map(|(index, unit)| {
                            let unit = unit.clone()?;
                            (environment.pack.units[&unit.unit_id].position != Position::Back)
                                .then_some((index, unit))
                        })?;
                Some((environment, front_index, front, bench_index, bench))
            })
            .expect("需要一组可交换的前台和备战席角色");
        environment
            .swap_unit_slots("front", front_index, "bench", bench_index)
            .unwrap();
        assert_eq!(environment.front_slots[front_index].as_ref(), Some(&bench));
        assert_eq!(environment.bench_slots[bench_index].as_ref(), Some(&front));
        let back_only = environment
            .pack
            .units
            .values()
            .find(|unit| unit.position == Position::Back)
            .unwrap()
            .id
            .clone();
        let candidate = UnitInstance {
            unit_id: back_only,
            star: 1,
        };
        environment.bench_slots[bench_index] = Some(candidate.clone());
        let empty_front = first_empty(&environment.front_slots).unwrap();
        assert!(
            environment
                .swap_unit_slots("bench", bench_index, "front", empty_front)
                .is_err()
        );
    }
}
