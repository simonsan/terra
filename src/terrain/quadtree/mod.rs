use cgmath::*;
use gfx;
use gfx::traits::FactoryExt;
use vecmath;

use rshader;

use std::env;
use std::collections::VecDeque;

use terrain::tile_cache::{Priority, TileCache};

pub(crate) mod id;
pub(crate) mod node;
pub(crate) mod render;

pub(crate) use terrain::quadtree::id::*;
pub(crate) use terrain::quadtree::node::*;
pub(crate) use terrain::quadtree::render::*;

const NUM_LAYERS: usize = 3;

pub struct QuadTree<R, F>
where
    R: gfx::Resources,
    F: gfx::Factory<R>,
{
    /// List of nodes in the `QuadTree`. The root is always at index 0.
    nodes: Vec<Node>,

    /// List of nodes that will be rendered.
    visible_nodes: Vec<NodeId>,

    /// Cache holding nearby tiles for each layer.
    tile_cache_layers: [TileCache; NUM_LAYERS],

    factory: F,
    pso: gfx::PipelineState<R, pipe::Meta>,
    pipeline_data: pipe::Data<R>,
    shaders_watcher: rshader::ShaderDirectoryWatcher,
    shader: rshader::Shader<R>,
}

#[allow(unused)]
impl<R, F> QuadTree<R, F>
where
    R: gfx::Resources,
    F: gfx::Factory<R>,
{
    pub fn new(
        mut factory: F,
        color_buffer: &gfx::handle::RenderTargetView<R, gfx::format::Srgba8>,
        depth_buffer: &gfx::handle::DepthStencilView<R, gfx::format::DepthStencil>,
    ) -> Self {
        Self::from_nodes(
            Node::make_nodes(524288.0, 3000.0, 13),
            factory,
            color_buffer,
            depth_buffer,
        )
    }

    pub(crate) fn from_nodes(
        nodes: Vec<Node>,
        mut factory: F,
        color_buffer: &gfx::handle::RenderTargetView<R, gfx::format::Srgba8>,
        depth_buffer: &gfx::handle::DepthStencilView<R, gfx::format::DepthStencil>,
    ) -> Self {
        let mut shaders_watcher = rshader::ShaderDirectoryWatcher::new(
            env::var("TERRA_SHADER_DIRECTORY").unwrap_or(".".to_string()),
        ).unwrap();

        let shader = rshader::Shader::simple(
            &mut factory,
            &mut shaders_watcher,
            shader_source!("../../shaders/glsl", "version", "terrain.glslv"),
            shader_source!("../../shaders/glsl", "version", "terrain.glslf"),
        ).unwrap();

        let heights_resolution = 5;

        Self {
            visible_nodes: Vec::new(),
            tile_cache_layers: [
                TileCache::new(1024, heights_resolution), // heights
                TileCache::new(512, 512), // normals
                TileCache::new(96, 512), // splats
            ],
            pso: Self::make_pso(&mut factory, shader.as_shader_set()),
            pipeline_data: pipe::Data {
                instances: factory.create_constant_buffer::<NodeState>(nodes.len()),
                model_view_projection: [[0.0; 4]; 4],
                resolution: heights_resolution as i32,
                color_buffer: color_buffer.clone(),
                depth_buffer: depth_buffer.clone(),
            },
            factory,
            shaders_watcher,
            shader,
            nodes,
        }
    }

    fn update_priorities(&mut self, camera: Point3<f32>) {
        for node in self.nodes.iter_mut() {
            node.priority = Priority::from_f32(
                (node.min_distance * node.min_distance) /
                    node.bounds.square_distance(camera).max(0.001),
            );
        }
        for cache_layer in self.tile_cache_layers.iter_mut() {
            cache_layer.update_priorities(&mut self.nodes);
        }
    }

    fn update_cache(&mut self) {
        self.breadth_first(|qt, id| {
            if qt.nodes[id].priority < Priority::cutoff() {
                return false;
            }

            for layer in 0..NUM_LAYERS {
                if qt.nodes[id].tile_indices[layer].is_some() &&
                    !qt.tile_cache_layers[layer].contains(id)
                {
                    qt.tile_cache_layers[layer].add_missing((qt.nodes[id].priority, id));
                }
            }
            true
        });
        for cache_layer in self.tile_cache_layers.iter_mut() {
            cache_layer.load_missing(&mut self.nodes);
        }
    }

    fn update_visibility(&mut self) {
        self.visible_nodes.clear();
        for node in self.nodes.iter_mut() {
            node.visible = false;
        }
        // Any node with all needed layers in cache is visible...
        self.breadth_first(|qt, id| {
            qt.nodes[id].visible = qt.nodes[id].priority >= Priority::cutoff() &&
                qt.nodes[id].tile_indices.iter().filter_map(|i| *i).all(
                    |i| {
                        qt.tile_cache_layers[i as usize].contains(id)
                    },
                );
            qt.nodes[id].visible
        });
        // ...Except if all its children are visible instead.
        self.breadth_first(|qt, id| if qt.nodes[id].visible {
            qt.nodes[id].visible = !qt.nodes[id].children.iter().all(|child| match *child {
                Some(c) => qt.nodes[c].visible,
                None => false,
            });
            if qt.nodes[id].visible {
                qt.visible_nodes.push(id);
            }
            true
        } else {
            false
        });
    }

    pub fn update(&mut self, mvp_mat: vecmath::Matrix4<f32>, camera: Point3<f32>) {
        self.update_priorities(camera);
        self.update_cache();
        self.update_visibility();
        self.update_shaders();

        self.pipeline_data.model_view_projection = mvp_mat;
    }

    fn breadth_first<Visit>(&mut self, mut visit: Visit)
    where
        Visit: FnMut(&mut Self, NodeId) -> bool,
    {
        let mut pending = VecDeque::new();
        if visit(self, NodeId::root()) {
            pending.push_back(NodeId::root());
        }
        while let Some(id) = pending.pop_front() {
            for i in 0..4 {
                if let Some(child) = self.nodes[id].children[i] {
                    if visit(self, child) {
                        pending.push_back(child);
                    }
                }
            }
        }
    }
}