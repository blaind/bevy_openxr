use bevy::{
    prelude::*,
    render::render_graph::{base::node, RenderGraph, WindowTextureNode},
};

use super::nodes::{XRSwapchainNode, XRWindowTextureNode};

pub(crate) fn add_xr_render_graph(mut graph: ResMut<RenderGraph>) {
    let main_depth_texture: &WindowTextureNode = graph.get_node(node::MAIN_DEPTH_TEXTURE).unwrap();
    let descriptor = *main_depth_texture.descriptor();

    graph
        .replace_node(
            node::MAIN_DEPTH_TEXTURE,
            XRWindowTextureNode::new(descriptor),
        )
        .unwrap();

    graph
        .replace_node(node::PRIMARY_SWAP_CHAIN, XRSwapchainNode::new())
        .unwrap();

    let main_sampled_color_attachment: &WindowTextureNode =
        graph.get_node(node::MAIN_SAMPLED_COLOR_ATTACHMENT).unwrap();

    let descriptor = *main_sampled_color_attachment.descriptor();

    graph
        .replace_node(
            node::MAIN_SAMPLED_COLOR_ATTACHMENT,
            XRWindowTextureNode::new(descriptor),
        )
        .unwrap();
}
