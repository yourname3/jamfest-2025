use crate::gc;

gc!(crate::video::world::World,    0xF0000000_u64);
gc!(crate::video::world::Viewport, 0xF0000001_u64);
gc!(crate::video::camera::Camera,  0xF0000002_u64);
gc!(crate::video::texture::Texture, 0xF0000006_u64);

gc!(crate::video::mesh_render_pipeline::Mesh, 0xF0000003_u64);
gc!(crate::video::mesh_render_pipeline::MeshInstance, 0xF0000004_u64);
// gc!(crate::video::mesh_render_pipeline::Material, 0xF0000005_u64);
gc!(crate::video::PBRMaterial, 0xF0000005_u64);
gc!(crate::video::PBRShader,   0xF0000008_u64);

gc!(wgpu::BindGroup, 0xE0000000_u64);