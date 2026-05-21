use re_entity_db::InstancePath;
use re_viewer_context::{Item, ViewId};

use crate::{ContextMenuAction, ContextMenuContext};

/// Right-click action: enable per-entity visibility linking across views.
pub(crate) struct LinkVisibilityAction;

impl ContextMenuAction for LinkVisibilityAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::DataResult(data_result) => {
                data_result_link_visibility(ctx, &data_result.view_id, &data_result.instance_path)
                    .is_some_and(|linked| !linked)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Link visibility across views (all)".to_owned()
        } else {
            "Link visibility across views".to_owned()
        }
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_link_visibility(ctx, view_id, instance_path, true);
    }
}

/// Right-click action: disable per-entity visibility linking.
pub(crate) struct UnlinkVisibilityAction;

impl ContextMenuAction for UnlinkVisibilityAction {
    fn supports_selection(&self, ctx: &ContextMenuContext<'_>) -> bool {
        ctx.selection.iter().any(|(item, _)| match item {
            Item::DataResult(data_result) => {
                data_result_link_visibility(ctx, &data_result.view_id, &data_result.instance_path)
                    .unwrap_or(false)
            }
            _ => false,
        })
    }

    fn label(&self, ctx: &ContextMenuContext<'_>) -> String {
        if ctx.selection.len() > 1 {
            "Unlink visibility across views (all)".to_owned()
        } else {
            "Unlink visibility across views".to_owned()
        }
    }

    fn process_data_result(
        &self,
        ctx: &ContextMenuContext<'_>,
        view_id: &ViewId,
        instance_path: &InstancePath,
    ) {
        set_data_result_link_visibility(ctx, view_id, instance_path, false);
    }
}

/// Returns `Some(true)` if linking is enabled, `Some(false)` if linking is disabled (or the
/// entity exists but has no flag), and `None` if there is no data result for this entity at all.
fn data_result_link_visibility(
    ctx: &ContextMenuContext<'_>,
    view_id: &ViewId,
    instance_path: &InstancePath,
) -> Option<bool> {
    if !instance_path.is_all() {
        return None;
    }
    let query_result = ctx.viewer_context.query_results.get(view_id)?;
    let data_result = query_result
        .tree
        .lookup_result_by_path(instance_path.entity_path.hash())?;
    Some(data_result.is_link_visibility_enabled(ctx.viewer_context))
}

fn set_data_result_link_visibility(
    ctx: &ContextMenuContext<'_>,
    view_id: &ViewId,
    instance_path: &InstancePath,
    linked: bool,
) {
    if let Some(query_result) = ctx.viewer_context.query_results.get(view_id) {
        if let Some(data_result) = query_result
            .tree
            .lookup_result_by_path(instance_path.entity_path.hash())
        {
            data_result.save_link_visibility(ctx.viewer_context, linked);
        }
    } else {
        re_log::error!("No query available for view {:?}", view_id);
    }
}
