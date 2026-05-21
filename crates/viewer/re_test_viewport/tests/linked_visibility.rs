//! Tests for per-entity visibility linking across views.
//!
//! When an entity's [`re_sdk_types::blueprint::components::LinkVisibility`] flag is set
//! (via [`re_viewer_context::DataResult::save_link_visibility`]), toggling visibility in
//! one view propagates the same `EntityBehavior(visible=…)` override to every other view
//! whose contents include that entity.

use re_chunk::TimePoint;
use re_entity_db::EntityPath;
use re_log_types::example_components::{MyPoint, MyPoints};
use re_sdk_types::Visualizer;
use re_sdk_types::blueprint::archetypes as blueprint_archetypes;
use re_sdk_types::components::Visible;
use re_test_context::{TestContext, VisualizerBlueprintContext as _};
use re_test_viewport::{TestContextExt as _, TestView};
use re_viewer_context::{RecommendedView, ViewClass as _, ViewId, ViewerContext};
use re_viewport_blueprint::{ViewBlueprint, ViewContents};

/// Toggling visibility on a linked entity should propagate the override to every peer view
/// whose query results contain that entity.
#[test]
fn toggling_visibility_on_linked_entity_propagates_to_peer_views() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();

    let entity_path = EntityPath::from("/world/points");
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });

    let view_ids: [ViewId; 3] = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let make_view = || {
            let view = ViewBlueprint::new_with_root_wildcard(TestView::identifier());
            ctx.save_visualizers(&entity_path, view.id, [Visualizer::new("Test")]);
            blueprint.add_view_at_root(view)
        };
        [make_view(), make_view(), make_view()]
    });

    enable_linking_and_toggle(&mut test_context, view_ids[0], &entity_path, false);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path =
                ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "view {view_id:?} should have linked visibility=false at {override_path}",
            );
        }
    });
}

/// Toggling a linked entity back to the parent default (so the originating view smart-clears
/// its own override) should also clear the override on every peer view.
#[test]
fn toggle_back_to_default_on_linked_entity_clears_all_peer_overrides() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();

    let entity_path = EntityPath::from("/world/points");
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });

    let view_ids: [ViewId; 3] = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let make_view = || {
            let view = ViewBlueprint::new_with_root_wildcard(TestView::identifier());
            ctx.save_visualizers(&entity_path, view.id, [Visualizer::new("Test")]);
            blueprint.add_view_at_root(view)
        };
        [make_view(), make_view(), make_view()]
    });

    enable_linking_and_toggle(&mut test_context, view_ids[0], &entity_path, false);

    // Now toggle back to visible. Originator smart-clears (parent default is visible);
    // peers should also be cleared, not left with explicit visible=true overrides.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let tree = &ctx.lookup_query_result(view_ids[0]).tree;
        let data_result = tree
            .lookup_result_by_path(entity_path.hash())
            .expect("view 0 should contain the entity");
        data_result.save_visible(ctx, tree, true);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        for view_id in view_ids {
            let override_path =
                ViewContents::base_override_path_for_entity(view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                None,
                "view {view_id:?} should have no override after toggle-back at {override_path}",
            );
        }
    });
}

/// Without enabling linking on the entity, toggling in one view must not touch any other
/// view's override. Pins the pre-existing per-view behavior.
#[test]
fn toggle_on_unlinked_entity_does_not_propagate() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();

    let entity_path = EntityPath::from("/world/points");
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });

    let view_ids: [ViewId; 3] = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        let make_view = || {
            let view = ViewBlueprint::new_with_root_wildcard(TestView::identifier());
            ctx.save_visualizers(&entity_path, view.id, [Visualizer::new("Test")]);
            blueprint.add_view_at_root(view)
        };
        [make_view(), make_view(), make_view()]
    });

    // No call to save_link_visibility — entity is not linked.
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let tree = &ctx.lookup_query_result(view_ids[0]).tree;
        let data_result = tree
            .lookup_result_by_path(entity_path.hash())
            .expect("view 0 should contain the entity");
        data_result.save_visible(ctx, tree, false);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let originating_path =
            ViewContents::base_override_path_for_entity(view_ids[0], &entity_path);
        assert_eq!(
            read_visible_override(ctx, &originating_path),
            Some(false),
            "originating view should have its override at {originating_path}",
        );
        for view_id in &view_ids[1..] {
            let override_path =
                ViewContents::base_override_path_for_entity(*view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                None,
                "peer view {view_id:?} should NOT have an override at {override_path}",
            );
        }
    });
}

/// Linking propagation only touches views whose `ViewContents` actually contains the entity.
#[test]
fn linking_skips_views_that_do_not_contain_the_entity() {
    let mut test_context = TestContext::new_with_view_class::<TestView>();

    let entity_path = EntityPath::from("/world/points");
    test_context.log_entity(entity_path.clone(), |b| {
        b.with_archetype_auto_row(
            TimePoint::STATIC,
            &MyPoints::new(vec![MyPoint::new(0.0, 0.0)]),
        )
    });

    let view_ids: [ViewId; 3] = test_context.setup_viewport_blueprint(|ctx, blueprint| {
        // Views 0 and 1 are rooted at /world and contain the entity.
        // View 2 is rooted at /cam — completely disjoint, so /world/points is not in its tree.
        let world_view = || {
            let view =
                ViewBlueprint::new(TestView::identifier(), RecommendedView::new_subtree("/world"));
            ctx.save_visualizers(&entity_path, view.id, [Visualizer::new("Test")]);
            blueprint.add_view_at_root(view)
        };
        let cam_view = || {
            let view =
                ViewBlueprint::new(TestView::identifier(), RecommendedView::new_subtree("/cam"));
            blueprint.add_view_at_root(view)
        };
        [world_view(), world_view(), cam_view()]
    });

    enable_linking_and_toggle(&mut test_context, view_ids[0], &entity_path, false);

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        // Views 0 and 1 (both rooted at /world) should both have the override.
        for view_id in &view_ids[..2] {
            let override_path =
                ViewContents::base_override_path_for_entity(*view_id, &entity_path);
            assert_eq!(
                read_visible_override(ctx, &override_path),
                Some(false),
                "world-rooted view {view_id:?} should have linked override at {override_path}",
            );
        }
        // View 2 (rooted at /cam) does not contain the entity — no override should be written.
        let cam_view_path = ViewContents::base_override_path_for_entity(view_ids[2], &entity_path);
        assert_eq!(
            read_visible_override(ctx, &cam_view_path),
            None,
            "cam-rooted view {:?} should NOT have an override at {cam_view_path}",
            view_ids[2],
        );
    });
}

/// Helper: enable linking on the entity (via view 0's data_result) and then toggle visibility
/// to `new_value`. Both writes are flushed via `handle_system_commands` so subsequent reads
/// see the resulting state.
fn enable_linking_and_toggle(
    test_context: &mut TestContext,
    view_id: ViewId,
    entity_path: &EntityPath,
    new_value: bool,
) {
    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let data_result = ctx
            .lookup_query_result(view_id)
            .tree
            .lookup_result_by_path(entity_path.hash())
            .cloned()
            .expect("originating view should contain the entity");
        data_result.save_link_visibility(ctx, true);
    });
    test_context.handle_system_commands(&egui::Context::default());

    test_context.run_in_egui_central_panel(|ctx, _ui| {
        let tree = &ctx.lookup_query_result(view_id).tree;
        let data_result = tree
            .lookup_result_by_path(entity_path.hash())
            .expect("originating view should contain the entity");
        data_result.save_visible(ctx, tree, new_value);
    });
    test_context.handle_system_commands(&egui::Context::default());
}

/// Read the `EntityBehavior.visible` component from the blueprint store at the given path.
///
/// Returns `None` if no override is set.
fn read_visible_override(ctx: &ViewerContext<'_>, path: &EntityPath) -> Option<bool> {
    let descriptor = blueprint_archetypes::EntityBehavior::descriptor_visible();
    let results =
        ctx.store_context
            .blueprint
            .latest_at(ctx.blueprint_query, path, [descriptor.component]);
    results
        .component_mono::<Visible>(descriptor.component)
        .map(|v| *v.0)
}
