use bevy::prelude::*;

#[derive(Component)]
pub struct PanelGroup {
    pub min_ratio: f32,
}

#[derive(Component)]
pub struct Panel {
    pub ratio: f32,
}

#[derive(Component)]
pub struct PanelHandle;

pub struct SplitPanelPlugin;

impl Plugin for SplitPanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(on_panel_added)
            .add_systems(Update, on_panel_changed);
    }
}

fn on_panel_added(
    trigger: On<Add, Panel>,
    child_of: Query<&ChildOf>,
    mut queries: ParamSet<(
        Query<(&Node, &Children), With<PanelGroup>>,
        Query<(&mut Node, &Panel)>,
    )>,
) {
    let entity = trigger.event_target();
    let Ok(&ChildOf(parent)) = child_of.get(entity) else {
        return;
    };
    recalculate_group(parent, &mut queries);
}

fn on_panel_changed(
    changed: Query<(Entity, &ChildOf), Changed<Panel>>,
    mut queries: ParamSet<(
        Query<(&Node, &Children), With<PanelGroup>>,
        Query<(&mut Node, &Panel)>,
    )>,
) {
    for (_, child_of) in &changed {
        recalculate_group(child_of.parent(), &mut queries);
    }
}

fn recalculate_group(
    group_entity: Entity,
    queries: &mut ParamSet<(
        Query<(&Node, &Children), With<PanelGroup>>,
        Query<(&mut Node, &Panel)>,
    )>,
) {
    // Pass 1: read group info and panel ratios
    let groups = queries.p0();
    let Ok((group_node, children)) = groups.get(group_entity) else {
        return;
    };
    let flex_direction = group_node.flex_direction;
    let child_entities: Vec<Entity> = children.iter().collect();

    let panels_ro = queries.p1();
    let total: f32 = panels_ro
        .iter_many(&child_entities)
        .map(|(_, panel)| panel.ratio)
        .sum();

    if total <= 0.0 {
        return;
    }

    // Pass 2: mutate panel nodes
    let mut panels = queries.p1();
    let mut iterator = panels.iter_many_mut(&child_entities);
    while let Some((mut node, panel)) = iterator.fetch_next() {
        let pct = (panel.ratio / total) * 100.;
        match flex_direction {
            FlexDirection::Row | FlexDirection::RowReverse => {
                node.width = percent(pct);
                node.min_width = px(0.0);
            }
            FlexDirection::Column | FlexDirection::ColumnReverse => {
                node.height = percent(pct);
                node.min_height = px(0.0);
            }
        }
    }
}
