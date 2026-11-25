use run_core::route::{NodeKind, RouteChange, RouteProgress, RouteStage, WrongRouteStage};

#[test]
fn advances_through_all_positions_with_mandatory_choices() {
    use RouteStage::*;

    let expected = [
        ("1-1", NodeEntered, 1),
        ("1-2", NodeEntered, 2),
        ("1-3", InvestmentChoice, 2),
        ("1-3", NodeEntered, 3),
        ("1-4", NodeEntered, 4),
        ("1-5", SupplyChoice, 4),
        ("1-6", NodeEntered, 5),
        ("1-7", NodeEntered, 6),
        ("1-8", NodeEntered, 7),
        ("1-9", NodeEntered, 8),
        ("2-1", NodeEntered, 9),
        ("2-2", InvestmentChoice, 9),
        ("2-2", NodeEntered, 10),
        ("2-3", SupplyChoice, 10),
        ("2-4", NodeEntered, 11),
        ("2-5", NodeEntered, 12),
        ("2-6", NodeEntered, 13),
        ("2-7", NodeEntered, 14),
        ("3-1", NodeEntered, 15),
        ("3-2", InvestmentChoice, 15),
        ("3-2", NodeEntered, 16),
        ("3-3", SupplyChoice, 16),
        ("3-4", NodeEntered, 17),
        ("3-5", NodeEntered, 18),
        ("3-6", NodeEntered, 19),
        ("3-7", NodeEntered, 20),
    ];

    let mut route = RouteProgress::default();
    for (id, stage, count) in expected {
        assert_eq!(
            (route.current().id, route.stage(), route.entered_nodes()),
            (id, stage, count)
        );
        match stage {
            NodeEntered => {
                let change = route.complete_node().unwrap();
                if id == "3-7" {
                    assert_eq!(change, RouteChange::Finished);
                }
            }
            InvestmentChoice => {
                assert_eq!(route.current().kind, NodeKind::Battle);
                assert_eq!(
                    route.complete_investment(),
                    Ok(RouteChange::Entered(route.current()))
                );
            }
            SupplyChoice => {
                assert_eq!(route.current().kind, NodeKind::Supply);
                let next = route.complete_supply().unwrap();
                assert_eq!(next, RouteChange::Entered(route.current()));
            }
            Finished => unreachable!("finished is checked after the final node"),
        }
    }
    assert_eq!(route.stage(), Finished);
    assert_eq!(route.entered_nodes(), 20);
    assert_eq!(
        route.complete_node(),
        Err(WrongRouteStage {
            expected: NodeEntered,
            actual: Finished
        })
    );
}

#[test]
fn cannot_skip_supply_or_enter_a_battle_before_choosing_an_investment() {
    let mut route = RouteProgress::default();
    route.complete_node().unwrap();
    let choice = route.complete_node().unwrap();
    assert_eq!(
        choice,
        RouteChange::ChoiceRequired {
            node: route.current(),
            stage: RouteStage::InvestmentChoice
        }
    );
    let before = route.clone();
    assert_eq!(
        route.complete_node(),
        Err(WrongRouteStage {
            expected: RouteStage::NodeEntered,
            actual: RouteStage::InvestmentChoice
        })
    );
    assert_eq!(route, before);

    route.complete_investment().unwrap();
    route.complete_node().unwrap();
    let choice = route.complete_node().unwrap();
    assert_eq!(
        choice,
        RouteChange::ChoiceRequired {
            node: route.current(),
            stage: RouteStage::SupplyChoice
        }
    );
    let before = route.clone();
    assert_eq!(
        route.complete_node(),
        Err(WrongRouteStage {
            expected: RouteStage::NodeEntered,
            actual: RouteStage::SupplyChoice
        })
    );
    assert_eq!(route, before);
}
