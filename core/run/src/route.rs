//! 4.7 S2 route positions and mandatory entry choices.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Reward,
    Battle,
    Encounter,
    Boss,
    Supply,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteNode {
    pub id: &'static str,
    pub kind: NodeKind,
    pub investment_before_entry: bool,
}

const fn node(id: &'static str, kind: NodeKind) -> RouteNode {
    RouteNode {
        id,
        kind,
        investment_before_entry: false,
    }
}

const fn investment_battle(id: &'static str) -> RouteNode {
    RouteNode {
        id,
        kind: NodeKind::Battle,
        investment_before_entry: true,
    }
}

pub const ROUTE: [RouteNode; 23] = {
    use NodeKind::*;
    [
        node("1-1", Reward),
        node("1-2", Reward),
        investment_battle("1-3"),
        node("1-4", Battle),
        node("1-5", Supply),
        node("1-6", Battle),
        node("1-7", Encounter),
        node("1-8", Reward),
        node("1-9", Boss),
        node("2-1", Battle),
        investment_battle("2-2"),
        node("2-3", Supply),
        node("2-4", Battle),
        node("2-5", Encounter),
        node("2-6", Reward),
        node("2-7", Boss),
        node("3-1", Battle),
        investment_battle("3-2"),
        node("3-3", Supply),
        node("3-4", Battle),
        node("3-5", Encounter),
        node("3-6", Reward),
        node("3-7", Boss),
    ]
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RouteStage {
    NodeEntered,
    InvestmentChoice,
    SupplyChoice,
    Finished,
}

/// Notifications for applying node-entry effects and updating the visible route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteChange {
    Entered(RouteNode),
    ChoiceRequired { node: RouteNode, stage: RouteStage },
    Finished,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrongRouteStage {
    pub expected: RouteStage,
    pub actual: RouteStage,
}

/// Route position after the opening environment has been selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteProgress {
    index: usize,
    stage: RouteStage,
    entered_nodes: usize,
}

impl Default for RouteProgress {
    fn default() -> Self {
        Self {
            index: 0,
            stage: RouteStage::NodeEntered,
            entered_nodes: 1,
        }
    }
}

impl RouteProgress {
    pub fn current(&self) -> RouteNode {
        ROUTE[self.index]
    }

    pub fn stage(&self) -> RouteStage {
        self.stage
    }

    /// Counts formal node entries, including the opening node and excluding supplies.
    pub fn entered_nodes(&self) -> usize {
        self.entered_nodes
    }

    /// Called after the selected strategy is applied. The result signals node entry.
    pub fn complete_investment(&mut self) -> Result<RouteChange, WrongRouteStage> {
        self.require_stage(RouteStage::InvestmentChoice)?;
        Ok(self.enter())
    }

    /// Called after one supply option has been granted.
    pub fn complete_supply(&mut self) -> Result<RouteChange, WrongRouteStage> {
        self.require_stage(RouteStage::SupplyChoice)?;
        Ok(self.advance())
    }

    /// Called after the currently entered node has settled.
    pub fn complete_node(&mut self) -> Result<RouteChange, WrongRouteStage> {
        self.require_stage(RouteStage::NodeEntered)?;
        Ok(self.advance())
    }

    fn require_stage(&self, expected: RouteStage) -> Result<(), WrongRouteStage> {
        if self.stage == expected {
            return Ok(());
        }
        Err(WrongRouteStage {
            expected,
            actual: self.stage,
        })
    }

    fn advance(&mut self) -> RouteChange {
        if self.index + 1 == ROUTE.len() {
            self.stage = RouteStage::Finished;
            return RouteChange::Finished;
        }

        self.index += 1;
        let node = self.current();
        self.stage = match node {
            RouteNode {
                kind: NodeKind::Supply,
                ..
            } => RouteStage::SupplyChoice,
            RouteNode {
                investment_before_entry: true,
                ..
            } => RouteStage::InvestmentChoice,
            _ => return self.enter(),
        };
        RouteChange::ChoiceRequired {
            node,
            stage: self.stage,
        }
    }

    fn enter(&mut self) -> RouteChange {
        self.stage = RouteStage::NodeEntered;
        self.entered_nodes += 1;
        RouteChange::Entered(self.current())
    }
}
